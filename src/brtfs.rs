use crate::Directory;
use crate::File;
use std::convert::TryInto;
use std::os::unix::fs::FileExt;

// The following code decodes the btrfs filesystem following the on-disk format spec
// https://btrfs.readthedocs.io/en/latest/dev/On-disk-format.html
//
// Unlike FAT/exFAT/ext4, btrfs has no fixed directory-entry region or simple
// block/cluster table. Everything (root tree, fs tree, chunk tree, extents,
// directory entries) lives inside generic copy-on-write B-trees, and logical
// addresses must be translated to physical addresses via the chunk tree
// before anything can be read. So "index_from_root"/"index" here walk real
// B-tree nodes instead of a flat table.

const BTRFS_SUPERBLOCK_OFFSET: u64 = 65536; // 64 KiB, primary superblock
const BTRFS_MAGIC: &[u8; 8] = b"_BHRfS_M";

const BTRFS_FS_TREE_OBJECTID: u64 = 5;      // default subvolume's tree
const BTRFS_FIRST_FREE_OBJECTID: u64 = 256; // root inode of a subvolume

const BTRFS_INODE_ITEM_TYPE: u8 = 1;
const BTRFS_ROOT_ITEM_TYPE: u8 = 132;
const BTRFS_DIR_INDEX_TYPE: u8 = 96;
const BTRFS_CHUNK_ITEM_TYPE: u8 = 228;

const BTRFS_FT_DIR: u8 = 2;

const BTRFS_NODE_HEADER_SIZE: usize = 101;

/// Parses the sys_chunk_array embedded in the superblock: a run of
/// (btrfs_key, btrfs_chunk) pairs covering the SYSTEM chunk(s), just enough
/// to bootstrap reading of the real chunk tree.
fn parse_chunk_array(arr: &[u8]) -> Vec<(u64, u64, u64)> {
    let mut map = Vec::new();
    let mut off = 0usize;
    while off + 17 <= arr.len() {
        let ktype = arr[off + 8];
        let logical = u64::from_le_bytes(arr[off + 9..off + 17].try_into().unwrap());
        if ktype != BTRFS_CHUNK_ITEM_TYPE {
            break;
        }
        let chunk = &arr[off + 17..];
        if chunk.len() < 48 {
            break;
        }
        let length = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let num_stripes = u16::from_le_bytes([chunk[44], chunk[45]]) as usize;
        if chunk.len() < 48 + num_stripes * 32 {
            break;
        }
        let physical = u64::from_le_bytes(chunk[56..64].try_into().unwrap());
        map.push((logical, length, physical));
        off += 17 + 48 + num_stripes * 32;
    }
    map
}

struct BtrfsDrive {
    file: std::fs::File,
    directories: Vec<Directory>,
    _volume_label: String,
    mounted_at: String,
    files: Vec<BtrfsFile>,
    ignored_dirs: Vec<String>,
    // btrfs-specific
    nodesize: u64,
    chunk_map: Vec<(u64, u64, u64)>, // (logical_start, length, physical_start)
    fs_tree_root: u64,               // physical byte offset of the default fs tree's root node
}

impl BtrfsDrive {
    fn new(drive: String, mounted_at: String, ignored_dirs: Vec<String>) -> Result<Self, u32> {
        let file = std::fs::File::open(drive);
        if file.is_err() {
            return Err(1);
        }
        let file = file.unwrap();

        // Read superblock (4096 bytes at offset 65536)
        let mut sb = vec![0u8; 4096];
        file.read_at(&mut sb, BTRFS_SUPERBLOCK_OFFSET).unwrap();

        // Verify magic
        if &sb[64..72] != BTRFS_MAGIC {
            return Err(100);
        }
        // assert_eq!(&sb[64..72], BTRFS_MAGIC, "Not a valid btrfs filesystem (bad magic)");

        let root_logical = u64::from_le_bytes(sb[80..88].try_into().unwrap());
        let chunk_root_logical = u64::from_le_bytes(sb[88..96].try_into().unwrap());
        let nodesize = u32::from_le_bytes(sb[148..152].try_into().unwrap()) as u64;
        let sys_chunk_array_size = u32::from_le_bytes(sb[160..164].try_into().unwrap()) as usize;
        let sys_chunk_array = &sb[811..811 + sys_chunk_array_size];

        let mut drive = BtrfsDrive {
            file,
            directories: Vec::new(),
            _volume_label: String::new(),
            mounted_at,
            files: Vec::new(),
            ignored_dirs,
            nodesize,
            chunk_map: parse_chunk_array(sys_chunk_array),
            fs_tree_root: 0,
        };

        // Walk the real chunk tree to discover every chunk (METADATA/DATA,
        // not just the bootstrap SYSTEM ones), replacing the bootstrap map.
        let chunk_root_physical = drive.translate(chunk_root_logical);
        let mut full_map = drive.chunk_map.clone();
        drive.walk_leaves(chunk_root_physical, &mut |_objectid, item_type, offset, data| {
            if item_type == BTRFS_CHUNK_ITEM_TYPE && data.len() >= 48 {
                let length = u64::from_le_bytes(data[0..8].try_into().unwrap());
                let num_stripes = u16::from_le_bytes([data[44], data[45]]) as usize;
                if data.len() >= 48 + num_stripes * 32 {
                    let physical = u64::from_le_bytes(data[56..64].try_into().unwrap());
                    full_map.push((offset, length, physical));
                }
            }
        });
        drive.chunk_map = full_map;

        // Walk the root tree to find the default subvolume's ROOT_ITEM,
        // which gives us the logical address of the fs tree's root node.
        let root_tree_physical = drive.translate(root_logical);
        let mut fs_tree_bytenr = 0u64;
        drive.walk_leaves(root_tree_physical, &mut |objectid, item_type, _offset, data| {
            if objectid == BTRFS_FS_TREE_OBJECTID && item_type == BTRFS_ROOT_ITEM_TYPE && data.len() >= 184 {
                // btrfs_root_item: 160-byte inode item, then generation(8) +
                // root_dirid(8), then bytenr(8) at offset 176
                fs_tree_bytenr = u64::from_le_bytes(data[176..184].try_into().unwrap());
            }
        });
        if fs_tree_bytenr == 0 {
            return Err(101);
        }
        drive.fs_tree_root = drive.translate(fs_tree_bytenr);

        Ok(drive)
    }

    fn read_bytes(&self, from: u64, size: u64) -> Vec<u8> {
        let mut b = vec![0u8; size as usize];
        self.file.read_at(&mut b, from).unwrap();
        b
    }

    /// Translate a logical (chunk-tree-mapped) address to a physical byte
    /// offset on disk, the btrfs equivalent of exfat's cluster_to_byte /
    /// ext4's inode_offset.
    fn translate(&self, logical: u64) -> u64 {
        for (start, length, physical) in &self.chunk_map {
            if logical >= *start && logical < *start + *length {
                return physical + (logical - start);
            }
        }
        logical
    }

    /// Recursively walks a B-tree node (root tree, chunk tree, or fs tree
    /// all share the same on-disk node/leaf format) and invokes `callback`
    /// with (objectid, item_type, key_offset, item_data) for every leaf item.
    fn walk_leaves<F: FnMut(u64, u8, u64, &[u8])>(&self, node_physical: u64, callback: &mut F) {
        let node = self.read_bytes(node_physical, self.nodesize);
        if node.len() < BTRFS_NODE_HEADER_SIZE {
            return;
        }
        let nritems = u32::from_le_bytes([node[96], node[97], node[98], node[99]]) as usize;
        let level = node[100];

        if level == 0 {
            // Leaf: nritems * btrfs_item { key(17), data_offset(4), data_size(4) }
            for i in 0..nritems {
                let item_off = BTRFS_NODE_HEADER_SIZE + i * 25;
                if item_off + 25 > node.len() {
                    break;
                }
                let objectid = u64::from_le_bytes(node[item_off..item_off + 8].try_into().unwrap());
                let item_type = node[item_off + 8];
                let key_offset = u64::from_le_bytes(node[item_off + 9..item_off + 17].try_into().unwrap());
                let data_off = u32::from_le_bytes(node[item_off + 17..item_off + 21].try_into().unwrap()) as usize;
                let data_size = u32::from_le_bytes(node[item_off + 21..item_off + 25].try_into().unwrap()) as usize;
                let start = BTRFS_NODE_HEADER_SIZE + data_off;
                let end = start + data_size;
                if end > node.len() {
                    continue;
                }
                callback(objectid, item_type, key_offset, &node[start..end]);
            }
        } else {
            // Internal node: nritems * btrfs_key_ptr { key(17), blockptr(8), generation(8) }
            for i in 0..nritems {
                let item_off = BTRFS_NODE_HEADER_SIZE + i * 33;
                if item_off + 33 > node.len() {
                    break;
                }
                let blockptr = u64::from_le_bytes(node[item_off + 17..item_off + 25].try_into().unwrap());
                let child_physical = self.translate(blockptr);
                self.walk_leaves(child_physical, callback);
            }
        }
    }

    /// Reads the INODE_ITEM for a given objectid out of the fs tree,
    /// returning (size, create_timestamp, last_modified_timestamp).
    fn read_inode_item(&self, objectid: u64) -> Option<(u64, i64, i64)> {
        let mut result = None;
        self.walk_leaves(self.fs_tree_root, &mut |oid, item_type, _offset, data| {
            if oid == objectid && item_type == BTRFS_INODE_ITEM_TYPE && data.len() >= 160 {
                let size = u64::from_le_bytes(data[16..24].try_into().unwrap());
                let ctime = u64::from_le_bytes(data[124..132].try_into().unwrap()) as i64;
                let mtime = u64::from_le_bytes(data[136..144].try_into().unwrap()) as i64;
                result = Some((size, ctime, mtime));
            }
        });
        result
    }

    fn index_from_root(mut self) -> Self {
        if self.mounted_at == "/" {
            self.directories.push(Directory {
                name: self.mounted_at.clone(),
            });
        } else {
            self.directories.push(Directory {
                name: self.mounted_at.clone() + "/",
            });
        }

        let mut new_files = Vec::new();
        self.parse_dir_entries(BTRFS_FIRST_FREE_OBJECTID, 0, &self.mounted_at.clone(), &mut new_files);
        for f in new_files {
            self.files.push(f);
        }
        self
    }

    fn index(&mut self, parent: &BtrfsFile, parent_dir_idx: u32) {
        let parent_path = self.directories[parent_dir_idx as usize].name.clone();
        let mut new_files = Vec::new();
        self.parse_dir_entries(parent.first_cluster, parent_dir_idx, &parent_path, &mut new_files);

        let mut subdirs = Vec::new();
        for f in new_files {
            self.files.push(f.clone());
            if f.is_dir {
                subdirs.push(f);
            }
        }
        for subdir in subdirs {
            let name = self.directories[subdir.parent as usize].name.clone() + &subdir.name + "/";
            self.directories.push(Directory { name });
            let new_idx = self.directories.len() as u32 - 1;
            self.index(&subdir.clone(), new_idx);
        }
    }

    /// Walks the fs tree collecting DIR_INDEX items belonging to `dir_objectid`,
    /// the btrfs equivalent of scanning an ext4/exFAT/FAT32 directory block.
    fn parse_dir_entries(&self, dir_objectid: u64, parent_dir_idx: u32, parent_path: &str, out: &mut Vec<BtrfsFile>) {
        self.walk_leaves(self.fs_tree_root, &mut |objectid, item_type, _offset, data| {
            if objectid != dir_objectid || item_type != BTRFS_DIR_INDEX_TYPE {
                return;
            }
            // btrfs_dir_item: location(key,17) + transid(8) + data_len(2) + name_len(2) + type(1) + name[]
            if data.len() < 30 {
                return;
            }
            let child_objectid = u64::from_le_bytes(data[0..8].try_into().unwrap());
            let name_len = u16::from_le_bytes([data[27], data[28]]) as usize;
            let file_type = data[29];
            if data.len() < 30 + name_len {
                return;
            }
            let name = String::from_utf8_lossy(&data[30..30 + name_len]).to_string();

            // Skip . and ..
            if name == "." || name == ".." {
                return;
            }

            let is_dir = file_type == BTRFS_FT_DIR;
            let mut full_path = parent_path.to_string() + &name;
            if is_dir {
                full_path += "/";
            }

            // Check ignored dirs
            let to_ignore = self.ignored_dirs.iter().any(|ig| full_path.starts_with(ig));
            if to_ignore {
                return;
            }

            let (size, create_timestamp, last_modified_timestamp) =
                self.read_inode_item(child_objectid).unwrap_or((0, 0, 0));

            out.push(BtrfsFile {
                name,
                parent: parent_dir_idx,
                size,
                is_dir,
                create_timestamp,
                last_modified_timestamp,
                first_cluster: child_objectid,
            });
        });
    }
}

/// A file, timestamps use unix epoch
#[derive(Debug, Default, Clone)]
struct BtrfsFile {
    name: String,
    parent: u32,
    size: u64,
    is_dir: bool,
    create_timestamp: i64,
    last_modified_timestamp: i64,
    first_cluster: u64, // reused as objectid (inode number) for btrfs
}

fn from_btrfs_files_to_files(btrfs_file: &BtrfsFile) -> File {
    File {
        name: btrfs_file.name.clone(),
        parent: btrfs_file.parent,
        size: btrfs_file.size,
        is_dir: btrfs_file.is_dir,
        create_timestamp: btrfs_file.create_timestamp,
        last_modified_timestamp: btrfs_file.last_modified_timestamp,
    }
}

pub fn is_drive_valid(drive: String) -> bool {
    use std::fs;
    let file = fs::File::open(drive);
    if file.is_err() {
        return false;
    }
    let file = file.unwrap();
    let mut sb = vec![0u8; 72];
    if file.read_at(&mut sb, BTRFS_SUPERBLOCK_OFFSET).is_err() {
        return false;
    }
    &sb[64..72] == BTRFS_MAGIC
}

pub fn index(drive: String, mounted_at: String, ignored_dirs: Vec<String>) -> Result<(Vec<File>, Vec<Directory>), u32> {
    let drive = BtrfsDrive::new(drive, mounted_at, ignored_dirs);
    if drive.is_err() {
        return Err(drive.err().unwrap());
    }
    let mut drive = drive.unwrap().index_from_root();
    for i in 0..drive.files.len() {
        if drive.files[i].is_dir {
            let name = drive.directories[drive.files[i].parent as usize].name.clone()
                + &drive.files[i].name
                + "/";
            drive.directories.push(Directory { name });
            drive.index(&drive.files[i].clone(), drive.directories.len() as u32 - 1);
        }
    }
    let mut output = Vec::new();
    for f in drive.files {
        output.push(from_btrfs_files_to_files(&f));
    }
    Ok((output, drive.directories))
}