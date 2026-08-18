use crate::Directory;
use crate::File;
use std::convert::TryInto;
use std::os::unix::fs::FileExt;

const BTRFS_SUPERBLOCK_OFFSET: u64 = 65536; // 64 KiB, primary superblock
const BTRFS_MAGIC: &[u8; 8] = b"_BHRfS_M";

const BTRFS_FIRST_FREE_OBJECTID: u64 = 256; // fallback root inode of a subvolume

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

/// /proc/self/mountinfo escapes paths using octal sequences such as \040.
/// Decode the three escapes relevant to mount paths.
fn decode_mountinfo_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            match &bytes[i + 1..i + 4] {
                b"040" => {
                    result.push(' ');
                    i += 4;
                    continue;
                }
                b"011" => {
                    result.push('\t');
                    i += 4;
                    continue;
                }
                b"134" => {
                    result.push('\\');
                    i += 4;
                    continue;
                }
                _ => {}
            }
        }

        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

/// Finds the Btrfs subvolume ID actually mounted at `mount_point`.
fn mounted_subvolumes(drive: String) -> Vec<(u64, String)> {
    let mut result = Vec::new();

    let mountinfo = match std::fs::read_to_string("/proc/self/mountinfo") {
        Ok(v) => v,
        Err(_) => return result,
    };

    for line in mountinfo.lines() {
        if !line.contains(drive.as_str()){
            continue;
        }
        let (left, right) = match line.split_once(" - ") {
            Some(v) => v,
            None => continue,
        };

        let mut left_parts = left.split_whitespace();
        let _mount_id = left_parts.next();
        let _parent_id = left_parts.next();
        let _major_minor = left_parts.next();
        let _root = left_parts.next();

        let mountpoint = match left_parts.next() {
            Some(v) => decode_mountinfo_path(v),
            None => continue,
        };

        let mut right_parts = right.split_whitespace();
        let fs_type = match right_parts.next() {
            Some(v) => v,
            None => continue,
        };

        let _source = right_parts.next();

        let options = match right_parts.next() {
            Some(v) => v,
            None => continue,
        };

        if fs_type != "btrfs" {
            continue;
        }

        for option in options.split(',') {
            if let Some(id) = option.strip_prefix("subvolid=") {
                if let Ok(id) = id.parse::<u64>() {
                    result.push((id, mountpoint.clone()));
                }
                break;
            }
        }
    }
    dbg!(&result);
    result
}

#[derive(Debug)]
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
    root_tree_root: u64,             // physical byte offset of the root tree's root node
    fs_tree_root: u64,               // physical byte offset of the selected subvolume's root node
    fs_tree_root_dirid: u64,         // selected subvolume's own top-level directory objectid
}

impl BtrfsDrive {
    fn new(drive: String, mounted_at: String, subvol_id: u64, ignored_dirs: Vec<String>) -> Result<Self, u32> {
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

        let root_logical = u64::from_le_bytes(sb[80..88].try_into().unwrap());
        let chunk_root_logical = u64::from_le_bytes(sb[88..96].try_into().unwrap());
        let nodesize = u32::from_le_bytes(sb[148..152].try_into().unwrap()) as u64;
        let sys_chunk_array_size = u32::from_le_bytes(sb[160..164].try_into().unwrap()) as usize;
        let sys_chunk_array = &sb[811..811 + sys_chunk_array_size];

        let mut drive = BtrfsDrive {
            file,
            directories: Vec::new(),
            _volume_label: String::new(),
            mounted_at: mounted_at.clone(),
            files: Vec::new(),
            ignored_dirs,
            nodesize,
            chunk_map: parse_chunk_array(sys_chunk_array),
            root_tree_root: 0,
            fs_tree_root: 0,
            fs_tree_root_dirid: BTRFS_FIRST_FREE_OBJECTID,
        };

        // Walk the real chunk tree to discover every chunk (METADATA/DATA,
        // not just the bootstrap SYSTEM ones), replacing the bootstrap map.
        // This tree is small (a handful of chunks) regardless of how big
        // the filesystem's contents are, so a full walk is fine here.
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

        drive.root_tree_root = drive.translate(root_logical);
        let fs_subvol_id = subvol_id;
        // Find the selected subvolume's ROOT_ITEM in the root tree.
        let mut fs_tree_bytenr = 0u64;
        let mut fs_tree_root_dirid = BTRFS_FIRST_FREE_OBJECTID;
        drive.search_range(drive.root_tree_root, fs_subvol_id, BTRFS_ROOT_ITEM_TYPE, 0, &mut |_oid, _itype, _offset, data| {
            if data.len() >= 184 {
                // btrfs_root_item: 160-byte inode item, then generation(8),
                // root_dirid(8) at offset 168, bytenr(8) at offset 176
                fs_tree_root_dirid = u64::from_le_bytes(data[168..176].try_into().unwrap());
                fs_tree_bytenr = u64::from_le_bytes(data[176..184].try_into().unwrap());
            }
            false // ROOT_ITEM keys are unique per objectid; stop after first match
        });
        // Find the selected subvolume's ROOT_ITEM in the root tree.
        let mut fs_tree_bytenr = 0u64;
        let mut fs_tree_root_dirid = BTRFS_FIRST_FREE_OBJECTID;
        drive.search_range(drive.root_tree_root, fs_subvol_id, BTRFS_ROOT_ITEM_TYPE, 0, &mut |_oid, _itype, _offset, data| {
            if data.len() >= 184 {
                // btrfs_root_item: 160-byte inode item, then generation(8),
                // root_dirid(8) at offset 168, bytenr(8) at offset 176
                fs_tree_root_dirid = u64::from_le_bytes(data[168..176].try_into().unwrap());
                fs_tree_bytenr = u64::from_le_bytes(data[176..184].try_into().unwrap());
            }
            false // ROOT_ITEM keys are unique per objectid; stop after first match
        });
        if fs_tree_bytenr == 0 {
            return Err(101);
        }
        drive.fs_tree_root = drive.translate(fs_tree_bytenr);
        drive.fs_tree_root_dirid = fs_tree_root_dirid;

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

    fn walk_leaves<F: FnMut(u64, u8, u64, &[u8])>(&self, node_physical: u64, callback: &mut F) {
        let node = self.read_bytes(node_physical, self.nodesize);
        if node.len() < BTRFS_NODE_HEADER_SIZE {
            return;
        }
        let nritems = u32::from_le_bytes([node[96], node[97], node[98], node[99]]) as usize;
        let level = node[100];

        if level == 0 {
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

    /// Descends from `node_physical` to the leaf that would contain
    /// `target` (objectid, type, offset), using binary search over each
    /// internal node's sorted key_ptrs instead of visiting every child.
    fn find_leaf(&self, node_physical: u64, target: (u64, u8, u64)) -> u64 {
        let node = self.read_bytes(node_physical, self.nodesize);
        if node.len() < BTRFS_NODE_HEADER_SIZE {
            return node_physical;
        }
        let nritems = u32::from_le_bytes([node[96], node[97], node[98], node[99]]) as usize;
        let level = node[100];
        if level == 0 || nritems == 0 {
            return node_physical;
        }

        // Find the largest index whose key <= target (btrfs guarantees the
        // leftmost child covers everything less than its own first key, so
        // this always picks the correct subtree to descend into).
        let mut lo = 0usize;
        let mut hi = nritems;
        let mut best = 0usize;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let off = BTRFS_NODE_HEADER_SIZE + mid * 33;
            if off + 33 > node.len() {
                hi = mid;
                continue;
            }
            let objectid = u64::from_le_bytes(node[off..off + 8].try_into().unwrap());
            let ktype = node[off + 8];
            let koffset = u64::from_le_bytes(node[off + 9..off + 17].try_into().unwrap());
            let key = (objectid, ktype, koffset);
            if key <= target {
                best = mid;
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        let off = BTRFS_NODE_HEADER_SIZE + best * 33;
        let blockptr = u64::from_le_bytes(node[off + 17..off + 25].try_into().unwrap());
        let child_physical = self.translate(blockptr);
        self.find_leaf(child_physical, target)
    }

    /// Yields every item with key (objectid, item_type, offset) where
    /// offset >= start_offset, in ascending offset order, stopping as soon
    /// as objectid or item_type no longer match. `callback` returns false
    /// to stop early once the caller has what it needs. This only touches
    /// the leaves that actually contain matching items (jumping straight
    /// there via binary search, re-descending from the root on each leaf
    /// boundary), instead of scanning the whole tree.
    fn search_range<F: FnMut(u64, u8, u64, &[u8]) -> bool>(
        &self,
        tree_root: u64,
        objectid: u64,
        item_type: u8,
        mut start_offset: u64,
        callback: &mut F,
    ) {
        loop {
            let leaf_physical = self.find_leaf(tree_root, (objectid, item_type, start_offset));
            let node = self.read_bytes(leaf_physical, self.nodesize);
            if node.len() < BTRFS_NODE_HEADER_SIZE {
                return;
            }
            let nritems = u32::from_le_bytes([node[96], node[97], node[98], node[99]]) as usize;
            let level = node[100];
            if level != 0 {
                return; // find_leaf always returns a leaf; defensive only
            }

            // Binary search within the leaf for the first item whose key
            // is >= (objectid, item_type, start_offset).
            let target = (objectid, item_type, start_offset);
            let mut lo = 0usize;
            let mut hi = nritems;
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                let item_off = BTRFS_NODE_HEADER_SIZE + mid * 25;
                if item_off + 25 > node.len() {
                    hi = mid;
                    continue;
                }
                let oid = u64::from_le_bytes(node[item_off..item_off + 8].try_into().unwrap());
                let itype = node[item_off + 8];
                let ioff = u64::from_le_bytes(node[item_off + 9..item_off + 17].try_into().unwrap());
                if (oid, itype, ioff) < target {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }

            let mut i = lo;
            let mut last_offset_seen: Option<u64> = None;
            while i < nritems {
                let item_off = BTRFS_NODE_HEADER_SIZE + i * 25;
                if item_off + 25 > node.len() {
                    break;
                }
                let oid = u64::from_le_bytes(node[item_off..item_off + 8].try_into().unwrap());
                let itype = node[item_off + 8];
                let ioff = u64::from_le_bytes(node[item_off + 9..item_off + 17].try_into().unwrap());
                if oid != objectid || itype != item_type {
                    return; // moved past the range we care about; done
                }
                let data_off = u32::from_le_bytes(node[item_off + 17..item_off + 21].try_into().unwrap()) as usize;
                let data_size = u32::from_le_bytes(node[item_off + 21..item_off + 25].try_into().unwrap()) as usize;
                let start = BTRFS_NODE_HEADER_SIZE + data_off;
                let end = start + data_size;
                if end > node.len() {
                    i += 1;
                    continue;
                }
                if !callback(oid, itype, ioff, &node[start..end]) {
                    return; // caller is done (e.g. found the one item it needed)
                }
                last_offset_seen = Some(ioff);
                i += 1;
            }

            // Leaf ended while items were still matching -- the range may
            // continue in the next leaf. Re-descend from the root just
            // past the last offset we saw.
            match last_offset_seen {
                Some(off) if off != u64::MAX => {
                    start_offset = off + 1;
                }
                _ => return,
            }
        }
    }

    /// Reads the INODE_ITEM for a given objectid out of a specific tree
    /// (identified by the physical offset of that tree's root node),
    /// returning (size, create_timestamp, last_modified_timestamp).
    fn read_inode_item(&self, tree_root: u64, objectid: u64) -> Option<(u64, i64, i64)> {
        let mut result = None;
        self.search_range(tree_root, objectid, BTRFS_INODE_ITEM_TYPE, 0, &mut |_oid, _itype, _offset, data| {
            if data.len() >= 160 {
                let size = u64::from_le_bytes(data[16..24].try_into().unwrap());
                let ctime = u64::from_le_bytes(data[124..132].try_into().unwrap()) as i64;
                let mtime = u64::from_le_bytes(data[136..144].try_into().unwrap()) as i64;
                result = Some((size, ctime, mtime));
            }
            false // INODE_ITEM keys have offset 0 and are unique per objectid
        });
        result
    }

    /// Resolves a subvolume/snapshot id to its own tree, by finding its
    /// ROOT_ITEM in the root tree. Returns (tree_root_physical, root_dirid)
    /// for that subvolume's tree, analogous to what `new()` does for the default subvolume.
    fn resolve_subvol_tree(&self, subvol_id: u64) -> Option<(u64, u64)> {
        let mut result = None;
        self.search_range(self.root_tree_root, subvol_id, BTRFS_ROOT_ITEM_TYPE, 0, &mut |_oid, _itype, _offset, data| {
            if data.len() >= 184 {
                let root_dirid = u64::from_le_bytes(data[168..176].try_into().unwrap());
                let bytenr = u64::from_le_bytes(data[176..184].try_into().unwrap());
                result = Some((self.translate(bytenr), root_dirid));
            }
            false
        });
        result
    }

    fn index_from_root(mut self) -> Self {
        let root_name = if self.mounted_at == "/" {
            "/".to_string()
        } else {
            self.mounted_at.clone() + "/"
        };

        self.directories.push(Directory {
            name: root_name.clone(),
        });

        let mut new_files = Vec::new();
        let fs_tree_root = self.fs_tree_root;
        let fs_tree_root_dirid = self.fs_tree_root_dirid;

        self.parse_dir_entries(
            fs_tree_root,
            fs_tree_root_dirid,
            0,
            &root_name,
            &mut new_files,
        );

        for f in new_files {
            self.files.push(f);
        }

        self
    }

    fn index(&mut self, parent: &BtrfsFile, parent_dir_idx: u32) {
        let parent_path = self.directories[parent_dir_idx as usize].name.clone();
        let mut new_files = Vec::new();
        self.parse_dir_entries(parent.tree_root, parent.first_cluster, parent_dir_idx, &parent_path, &mut new_files);
        let mut subdirs = Vec::new();
        for f in new_files {
            self.files.push(f.clone());
            if f.is_dir {
                subdirs.push(f);
            }
        }
        for subdir in subdirs {
            let name = self.directories[subdir.parent as usize].name.clone() + &subdir.name + "/";
            self.directories.push(Directory { name: name.clone() });
            let new_idx = self.directories.len() as u32 - 1;
            self.index(&subdir.clone(), new_idx);
        }
    }

    /// Collects DIR_INDEX items belonging to `dir_objectid` within
    /// `tree_root`, the btrfs equivalent of scanning an ext4/exFAT/FAT32
    /// directory block. Uses `search_range` so this only touches the leaves holding
    /// this directory's entries, not the whole tree.
    fn parse_dir_entries(&self, tree_root: u64, dir_objectid: u64, parent_dir_idx: u32, parent_path: &str, out: &mut Vec<BtrfsFile>) {
        self.search_range(tree_root, dir_objectid, BTRFS_DIR_INDEX_TYPE, 0, &mut |_objectid, _item_type, _offset, data| {
            // btrfs_dir_item: location(key,17) + transid(8) + data_len(2) + name_len(2) + type(1) + name[]
            // The location key itself is objectid(8) + type(1) + offset(8).
            if data.len() < 30 {
                return true;
            }
            let location_objectid = u64::from_le_bytes(data[0..8].try_into().unwrap());
            let location_type = data[8];
            let name_len = u16::from_le_bytes([data[27], data[28]]) as usize;
            let file_type = data[29];
            if data.len() < 30 + name_len {
                return true;
            }
            let name = String::from_utf8_lossy(&data[30..30 + name_len]).to_string();

            if name == "." || name == ".." {
                return true;
            }

            let is_dir_by_type = file_type == BTRFS_FT_DIR;
            let mut full_path = parent_path.to_string();
            if is_dir_by_type {
                full_path = full_path + &name + "/";
            } else {
                full_path += &name;
            }

            if self.ignored_dirs.iter().any(|ig| full_path.starts_with(ig)) {
                return true;
            }

            // A DIR_INDEX entry whose location key is a ROOT_ITEM (rather
            // than an INODE_ITEM) is a subvolume or snapshot mount point.
            // `location_objectid` in that case is a subvolume id, living
            // in a separate id space from ordinary inode numbers -- it has
            // to be resolved via the root tree to find that subvolume's
            // own tree before it can be walked.
            let (child_tree_root, child_objectid, is_dir) = if location_type == BTRFS_ROOT_ITEM_TYPE {
                match self.resolve_subvol_tree(location_objectid) {
                    Some((subvol_tree_root, subvol_root_dirid)) => (subvol_tree_root, subvol_root_dirid, true),
                    None => return true, // couldn't resolve; skip rather than guess
                }
            } else if location_type == BTRFS_INODE_ITEM_TYPE {
                (tree_root, location_objectid, is_dir_by_type)
            } else {
                return true; // unrecognized location key type; skip
            };

            let (size, create_timestamp, last_modified_timestamp) =
                self.read_inode_item(child_tree_root, child_objectid).unwrap_or((0, 0, 0));

            out.push(BtrfsFile {
                name,
                parent: parent_dir_idx,
                size,
                is_dir,
                create_timestamp,
                last_modified_timestamp,
                first_cluster: child_objectid,
                tree_root: child_tree_root,
            });
            true
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
    first_cluster: u64, // reused as objectid (inode number) for btrfs, within `tree_root`'s tree
    tree_root: u64,     // physical offset of the B-tree this entry's objectid belongs to
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

pub fn index(drive: String, _mounted_at: String, ignored_dirs: Vec<String>) -> Result<(Vec<File>, Vec<Directory>), u32> {
    let subvolumes = mounted_subvolumes(drive.clone());

    let mut output = Vec::new();
    let mut directories = Vec::new();

    for (subvol_id, mounted_at) in subvolumes {
        let drive = BtrfsDrive::new(
            drive.clone(),
            mounted_at,
            subvol_id,
            ignored_dirs.clone(),
        );
        if drive.is_err() {
            continue;
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

        for mut f in drive.files {
            f.parent += directories.len() as u32;
            output.push(from_btrfs_files_to_files(&f));
        }

        directories.extend(drive.directories);
    }

    Ok((output, directories))
}