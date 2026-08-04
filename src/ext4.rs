use crate::Directory;
use crate::File;
use std::os::unix::fs::FileExt;

// The following code decodes the ext4 filesystem following the ext4 spec
// https://www.kernel.org/doc/html/latest/filesystems/ext4/index.html

const EXT4_SUPERBLOCK_OFFSET: u64 = 1024;
const EXT4_ROOT_INO: u32 = 2;
// const EXT4_FT_UNKNOWN: u8 = 0;
// const EXT4_FT_REG_FILE: u8 = 1;
const EXT4_FT_DIR: u8 = 2;

struct Ext4Drive {
    file: std::fs::File,
    directories: Vec<Directory>,
    volume_label: String,
    mounted_at: String,
    bytes_per_sector: u64,
    sectors_per_cluster: u64,
    cluster_size: u64,       // block size
    root_dir_cluster: u64,   // root inode number
    files: Vec<Ext4File>,
    ignored_dirs: Vec<String>,
    // ext4-specific
    inodes_per_group: u32,
    inode_size: u32,
    blocks_per_group: u32,
    block_size: u64,
    desc_size: u32,          // group descriptor size (32 or 64 bytes)
    s_desc_size: u32,        // from superblock field
}

impl Ext4Drive {
    fn new(drive: String, mounted_at: String, ignored_dirs: Vec<String>) -> Result<Self, u32> {
        let file = std::fs::File::open(drive);
        if file.is_err(){
            return Err(1);
        }
        let file = file.unwrap();

        // Read superblock (1024 bytes at offset 1024)
        let mut sb = vec![0u8; 1024];
        file.read_at(&mut sb, EXT4_SUPERBLOCK_OFFSET).unwrap();

        // Verify magic
        let magic = u16::from_le_bytes([sb[56], sb[57]]);
        if magic != 0xEF53{
            return Err(100);
        }
        // assert_eq!(magic, 0xEF53, "Not a valid ext4 filesystem (bad magic)");

        let block_size = 1024u64 << u32::from_le_bytes([sb[24], sb[25], sb[26], sb[27]]);
        let blocks_per_group = u32::from_le_bytes([sb[32], sb[33], sb[34], sb[35]]);
        let inodes_per_group = u32::from_le_bytes([sb[40], sb[41], sb[42], sb[43]]);
        let inode_size = u32::from_le_bytes([sb[88], sb[89], sb[90], sb[91]]);
        let s_desc_size = u32::from_le_bytes([sb[254], sb[255], sb[256], sb[257]]);

        // RO_COMPAT_64BIT flag => descriptor size 64 bytes, else 32
        let ro_compat = u32::from_le_bytes([sb[100], sb[101], sb[102], sb[103]]);
        let desc_size = if ro_compat & 0x0002 != 0 && s_desc_size >= 64 { 64u32 } else { 32u32 };

        // Volume label: bytes 120..136, UTF-8
        let label_bytes = &sb[120..136];
        let label_end = label_bytes.iter().position(|&b| b == 0).unwrap_or(16);
        let volume_label = String::from_utf8_lossy(&label_bytes[..label_end]).to_string();

        Ok(Ext4Drive {
            file,
            directories: Vec::new(),
            volume_label,
            mounted_at,
            bytes_per_sector: 512,
            sectors_per_cluster: block_size / 512,
            cluster_size: block_size,
            root_dir_cluster: EXT4_ROOT_INO as u64,
            files: Vec::new(),
            ignored_dirs,
            inodes_per_group,
            inode_size,
            blocks_per_group,
            block_size,
            desc_size,
            s_desc_size,
        })
    }

    fn read_bytes(&self, from: u64, size: u64) -> Vec<u8> {
        let mut b = vec![0u8; size as usize];
        self.file.read_at(&mut b, from).unwrap();
        b
    }

    /// Returns byte offset of the group descriptor for the given block group
    fn group_desc_offset(&self, group: u32) -> u64 {
        // Group descriptors start at block 1 when block_size == 1024, else block 1 too
        // Actually: first block after superblock block
        let sb_block = if self.block_size == 1024 { 1u64 } else { 0u64 };
        (sb_block + 1) * self.block_size + (group as u64) * self.desc_size as u64
    }

    /// Returns byte offset of inode in the filesystem image
    fn inode_offset(&self, inode_num: u32) -> u64 {
        let group = (inode_num - 1) / self.inodes_per_group;
        let index = (inode_num - 1) % self.inodes_per_group;

        let gd_offset = self.group_desc_offset(group);
        let gd = self.read_bytes(gd_offset, self.desc_size as u64);

        // inode_table block: bytes 8..12 (lo), bytes 40..44 (hi if 64-bit)
        let table_lo = u32::from_le_bytes([gd[8], gd[9], gd[10], gd[11]]) as u64;
        let table_hi = if self.desc_size >= 64 {
            u32::from_le_bytes([gd[40], gd[41], gd[42], gd[43]]) as u64
        } else {
            0u64
        };
        let table_block = (table_hi << 32) | table_lo;
        table_block * self.block_size + index as u64 * self.inode_size as u64
    }

    /// Read an inode and return raw bytes
    fn read_inode(&self, inode_num: u32) -> Vec<u8> {
        let offset = self.inode_offset(inode_num);
        self.read_bytes(offset, self.inode_size as u64)
    }

    /// Collect all data blocks for an inode using the extent tree
    /// Returns blocks in logical order as a flat Vec<u8>
    fn read_inode_data(&self, inode: &[u8], file_size: u64) -> Vec<u8> {
        // i_flags at offset 32
        let flags = u32::from_le_bytes([inode[32], inode[33], inode[34], inode[35]]);
        let uses_extents = flags & 0x80000 != 0;

        if uses_extents {
            // The 60-byte block array (offset 40) is actually the extent tree root
            let extent_data = &inode[40..100];
            let mut data = Vec::new();
            self.read_extent_tree(extent_data, &mut data, file_size);
            data
        } else {
            // Classic block map (not common in ext4 but handle gracefully)
            self.read_block_map(inode, file_size)
        }
    }

    fn read_extent_tree(&self, data: &[u8], out: &mut Vec<u8>, file_size: u64) {
        let magic = u16::from_le_bytes([data[0], data[1]]);
        assert_eq!(magic, 0xF30A, "Bad extent header magic");
        let entries = u16::from_le_bytes([data[2], data[3]]) as usize;
        let depth = u16::from_le_bytes([data[6], data[7]]);

        if depth == 0 {
            // Leaf node: each entry is 12 bytes starting at offset 12
            for i in 0..entries {
                let e = &data[12 + i * 12..12 + (i + 1) * 12];
                let len = u16::from_le_bytes([e[4], e[5]]) as u64;
                let start_hi = u16::from_le_bytes([e[6], e[7]]) as u64;
                let start_lo = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as u64;
                let phys_block = (start_hi << 32) | start_lo;
                let byte_offset = phys_block * self.block_size;
                let read_size = (len * self.block_size).min(file_size.saturating_sub(out.len() as u64));
                if read_size == 0 { break; }
                let mut block_data = self.read_bytes(byte_offset, len * self.block_size);
                block_data.truncate(read_size as usize);
                out.append(&mut block_data);
                if out.len() as u64 >= file_size { break; }
            }
        } else {
            // Index node: each entry is 12 bytes, recurse into child blocks
            for i in 0..entries {
                let e = &data[12 + i * 12..12 + (i + 1) * 12];
                let leaf_lo = u32::from_le_bytes([e[4], e[5], e[6], e[7]]) as u64;
                let leaf_hi = u16::from_le_bytes([e[8], e[9]]) as u64;
                let child_block = (leaf_hi << 32) | leaf_lo;
                let child_data = self.read_bytes(child_block * self.block_size, self.block_size);
                self.read_extent_tree(&child_data, out, file_size);
                if out.len() as u64 >= file_size { break; }
            }
        }
    }

    /// Fallback: read classic (non-extent) block map
    fn read_block_map(&self, inode: &[u8], file_size: u64) -> Vec<u8> {
        let mut out = Vec::new();
        // Direct blocks: offsets 40..79 (15 block pointers, 4 bytes each)
        // We only handle direct blocks here for simplicity
        for i in 0..12usize {
            let block = u32::from_le_bytes([
                inode[40 + i * 4],
                inode[41 + i * 4],
                inode[42 + i * 4],
                inode[43 + i * 4],
            ]) as u64;
            if block == 0 { break; }
            let read_size = self.block_size.min(file_size.saturating_sub(out.len() as u64));
            if read_size == 0 { break; }
            let mut block_data = self.read_bytes(block * self.block_size, self.block_size);
            block_data.truncate(read_size as usize);
            out.append(&mut block_data);
        }
        // Indirect blocks are skipped for simplicity (uncommon in ext4)
        out
    }

    fn index_from_root(mut self) -> Self {
        if self.mounted_at == "/"{
            self.directories.push(Directory {
                name: self.mounted_at.clone(),
            });
        }else{
            self.directories.push(Directory {
                name: self.mounted_at.clone() + "/",
            });
        }
        let root_inode = self.read_inode(EXT4_ROOT_INO);
        let file_size = u64::from_le_bytes([
            root_inode[4], root_inode[5], root_inode[6], root_inode[7], // lo
            root_inode[108], root_inode[109], root_inode[110], root_inode[111], // hi
        ]);
        let data = self.read_inode_data(&root_inode, file_size);

        let mut new_files = Vec::new();
        self.parse_dir_entries(&data, 0 /* parent dir index */, &self.mounted_at.clone(), &mut new_files);
        for f in new_files {
            self.files.push(f);
        }
        self
    }

    fn index(&mut self, parent: &Ext4File, parent_dir_idx: u32) {
        let inode = self.read_inode(parent.first_cluster);
        let file_size = u64::from_le_bytes([
            inode[4], inode[5], inode[6], inode[7],
            inode[108], inode[109], inode[110], inode[111],
        ]);
        let data = self.read_inode_data(&inode, file_size);

        let parent_path = self.directories[parent_dir_idx as usize].name.clone();
        let mut new_files = Vec::new();
        self.parse_dir_entries(&data, parent_dir_idx, &parent_path, &mut new_files);

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

    fn parse_dir_entries(
        &self,
        data: &[u8],
        parent_dir_idx: u32,
        parent_path: &str,
        out: &mut Vec<Ext4File>,
    ) {
        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let inode_num = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            ]);
            let rec_len = u16::from_le_bytes([data[offset + 4], data[offset + 5]]) as usize;
            if rec_len == 0 { break; }
            if inode_num == 0 {
                offset += rec_len;
                continue;
            }
            let name_len = data[offset + 6] as usize;
            let file_type = data[offset + 7];

            if name_len == 0 || offset + 8 + name_len > data.len() {
                offset += rec_len;
                continue;
            }
            let name_bytes = &data[offset + 8..offset + 8 + name_len];
            let name = String::from_utf8_lossy(name_bytes).to_string();

            // Skip . and ..
            if name == "." || name == ".." {
                offset += rec_len;
                continue;
            }

            let is_dir = file_type == EXT4_FT_DIR;
            let mut full_path = parent_path.to_string() + &name;
            if is_dir {
                full_path += "/";
            }

            // Check ignored dirs
            let to_ignore = self.ignored_dirs.iter().any(|ig| full_path.starts_with(ig));
            if to_ignore {
                offset += rec_len;
                continue;
            }

            // Read inode for timestamps and size
            let child_inode = self.read_inode(inode_num);
            let size = u64::from_le_bytes([
                child_inode[4], child_inode[5], child_inode[6], child_inode[7],
                child_inode[108], child_inode[109], child_inode[110], child_inode[111],
            ]);

            // Timestamps: i_atime=0x8, i_ctime=0xC, i_mtime=0x10 (32-bit seconds since epoch)
            // Extra ns in i_ctime_extra, i_mtime_extra, i_atime_extra at offsets 128, 132, 136
            // For simplicity we use 32-bit seconds
            let create_timestamp = u32::from_le_bytes([
                child_inode[12], child_inode[13], child_inode[14], child_inode[15],
            ]) as i64;
            let last_modified_timestamp = u32::from_le_bytes([
                child_inode[16], child_inode[17], child_inode[18], child_inode[19],
            ]) as i64;

            out.push(Ext4File {
                name,
                parent: parent_dir_idx,
                size,
                is_dir,
                create_timestamp,
                last_modified_timestamp,
                first_cluster: inode_num,
                // contigous: false, // extent tree handles this
            });

            offset += rec_len;
        }
    }
}

/// A file, timestamps use unix epoch
#[derive(Debug, Default, Clone)]
struct Ext4File {
    name: String,
    parent: u32,
    size: u64,
    is_dir: bool,
    create_timestamp: i64,
    last_modified_timestamp: i64,
    first_cluster: u32, // reused as inode number for ext4
    // contigous: bool,
}

fn from_ext4_files_to_files(ext4_file: &Ext4File, idx: u32) -> File {
    File {
        name: ext4_file.name.clone(),
        parent: ext4_file.parent + idx,
        size: ext4_file.size,
        is_dir: ext4_file.is_dir,
        create_timestamp: ext4_file.create_timestamp,
        last_modified_timestamp: ext4_file.last_modified_timestamp,
    }
}
pub fn is_drive_valid(drive: String) -> bool{
    use std::fs;
    let file = fs::File::open(drive);
    if file.is_err(){return false}
    let file = file.unwrap();
    let mut sb = vec![0u8; 1024];
    file.read_at(&mut sb, EXT4_SUPERBLOCK_OFFSET).unwrap();
    let magic = u16::from_le_bytes([sb[56], sb[57]]);
    if magic == 0xEF53{
        true
    }else{
        false
    }
}
pub fn index(drive: String, mounted_at: String, ignored_dirs: Vec<String>, idx: u32) -> Result<(Vec<File>, Vec<Directory>), u32> {
    let drive = Ext4Drive::new(drive, mounted_at, ignored_dirs);
    if drive.is_err(){
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
        output.push(from_ext4_files_to_files(&f, idx));
    }
    Ok((output, drive.directories))
}
