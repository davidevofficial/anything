use crate::Directory;
use crate::File;
use std::os::unix::fs::FileExt;
use std::fs;
// The following code decodes the NTFS filesystem by reading the MFT directly.
//   https://flatcap.github.io/linux-ntfs/ntfs/index.html
//   https://learn.microsoft.com/en-us/windows/win32/fileio/master-file-table

const _MFT_RECORD_MFT:       u64 = 0;
const MFT_RECORD_ROOT:      u64 = 5;
const ATTR_STANDARD_INFORMATION: u32 = 0x10;
const ATTR_FILE_NAME:            u32 = 0x30;
const ATTR_DATA:                 u32 = 0x80;
const _ATTR_INDEX_ROOT:           u32 = 0x90;
const _ATTR_INDEX_ALLOCATION:     u32 = 0xA0;
const ATTR_END:                  u32 = 0xFFFF_FFFF;
const MFT_RECORD_IN_USE:    u16 = 0x0001;
const MFT_RECORD_IS_DIR:    u16 = 0x0002;
const NAMESPACE_POSIX:  u8 = 0;
const NAMESPACE_WIN32:  u8 = 1;
const NAMESPACE_DOS:    u8 = 2;
const NAMESPACE_WIN32DOS: u8 = 3;

// Convert a Windows FILETIME (100-ns intervals since 1601-01-01) to Unix epoch seconds
fn filetime_to_epoch(ft: u64) -> i64 {
    const EPOCH_DIFF: u64 = 116_444_736_000_000_000;
    if ft < EPOCH_DIFF { return 0; }
    ((ft - EPOCH_DIFF) / 10_000_000) as i64
}

fn decode_run_list(data: &[u8]) -> Vec<(Option<u64>, u64)> {
    let mut runs = Vec::new();
    let mut pos = 0usize;
    let mut prev_lcn: i64 = 0;

    while pos < data.len() {
        let header = data[pos];
        if header == 0x00 { break; }
        pos += 1;

        let len_bytes  = (header & 0x0F) as usize;
        let off_bytes  = (header >> 4)   as usize;

        if len_bytes == 0 { break; }
        if pos + len_bytes + off_bytes > data.len() { break; }

        // Length field (unsigned)
        let mut run_len = 0u64;
        for i in 0..len_bytes {
            run_len |= (data[pos + i] as u64) << (i * 8);
        }
        pos += len_bytes;

        if off_bytes == 0 {
            // Sparse run
            runs.push((None, run_len));
        } else {
            // Offset is signed, relative to previous LCN
            let mut raw_off = 0i64;
            for i in 0..off_bytes {
                raw_off |= (data[pos + i] as i64) << (i * 8);
            }
            // Sign-extend
            let sign_bit = 1i64 << (off_bytes * 8 - 1);
            if raw_off & sign_bit != 0 {
                raw_off |= !((sign_bit << 1) - 1);
            }
            pos += off_bytes;
            prev_lcn += raw_off;
            runs.push((Some(prev_lcn as u64), run_len));
        }
    }
    runs
}

struct NtfsDrive {
    file:                  std::fs::File,
    directories:           Vec<Directory>,
    volume_label:          String,
    mounted_at:            String,
    bytes_per_sector:      u64,
    sectors_per_cluster:   u64,
    cluster_size:          u64,
    mft_record_size:       u64,
    mft_runs:              Vec<(Option<u64>, u64)>,
    files:                 Vec<NtfsFile>,
    ignored_dirs:          Vec<String>,
}

impl NtfsDrive {
    fn new(drive: String, mounted_at: String, ignored_dirs: Vec<String>) -> Result<Self, u32> {
        let file = fs::File::open(drive);
        if file.is_err(){
            return Err(1);
        }
        let file = file.unwrap();
        let mut vbr = vec![0u8; 512];
        file.read_at(&mut vbr, 0).unwrap();

        // magic bytes "NTFS    "
        if &vbr[3..11] != b"NTFS    "{
             return Err(100);
        }
        // assert_eq!(&vbr[3..11], b"NTFS    ", "Not a valid NTFS volume (bad OEM ID)");

        let bytes_per_sector    = u16::from_le_bytes([vbr[11], vbr[12]]) as u64;
        let sectors_per_cluster = vbr[13] as u64;
        let cluster_size        = bytes_per_sector * sectors_per_cluster;


        let mft_cluster = u64::from_le_bytes([
            vbr[48],vbr[49],vbr[50],vbr[51],vbr[52],vbr[53],vbr[54],vbr[55],
        ]);

        let clusters_per_mft_raw = vbr[64] as i8;
        let mft_record_size = if clusters_per_mft_raw < 0 {
            1u64 << (-clusters_per_mft_raw as u64)
        } else {
            clusters_per_mft_raw as u64 * cluster_size
        };

        let mft_start_byte = mft_cluster * cluster_size;
        let mut mft_file_record = Self::static_read_bytes(&file, mft_start_byte, mft_record_size);
        Self::fixup_record(&mut mft_file_record, bytes_per_sector);

        let mft_runs = Self::get_data_runs(&mft_file_record, mft_record_size);

        Ok(NtfsDrive {
            file,
            directories: Vec::new(),
            volume_label: String::new(),
            mounted_at,
            bytes_per_sector,
            sectors_per_cluster,
            cluster_size,
            mft_record_size,
            mft_runs,
            files: Vec::new(),
            ignored_dirs,
        })
    }

    fn static_read_bytes(file: &std::fs::File, from: u64, size: u64) -> Vec<u8> {
        let mut b = vec![0u8; size as usize];
        file.read_at(&mut b, from).unwrap();
        b
    }

    fn read_bytes(&self, from: u64, size: u64) -> Vec<u8> {
        Self::static_read_bytes(&self.file, from, size)
    }

    fn lcn_to_byte(&self, lcn: u64) -> u64 {
        lcn * self.cluster_size
    }

    fn fixup_record(record: &mut Vec<u8>, bytes_per_sector: u64) {
        if record.len() < 8 { return; }
        let usa_offset = u16::from_le_bytes([record[4], record[5]]) as usize;
        let usa_count  = u16::from_le_bytes([record[6], record[7]]) as usize;
        if usa_offset + usa_count * 2 > record.len() { return; }
        for i in 1..usa_count {
            let sector_end = i * bytes_per_sector as usize - 2;
            if sector_end + 1 >= record.len() { break; }
            record[sector_end]     = record[usa_offset + i * 2];
            record[sector_end + 1] = record[usa_offset + i * 2 + 1];
        }
    }

    /// Read the nth MFT record (0-based) using the MFT run list.
    fn read_mft_record(&self, index: u64) -> Vec<u8> {
        let byte_offset_in_mft = index * self.mft_record_size;
        // Walk the run list to find which run covers this offset
        let mut run_start = 0u64;
        for (lcn_opt, len_clusters) in &self.mft_runs {
            let run_len_bytes = len_clusters * self.cluster_size;
            if byte_offset_in_mft < run_start + run_len_bytes {
                let offset_in_run = byte_offset_in_mft - run_start;
                match lcn_opt {
                    Some(lcn) => {
                        let phys = self.lcn_to_byte(*lcn) + offset_in_run;
                        let mut rec = self.read_bytes(phys, self.mft_record_size);
                        Self::fixup_record(&mut rec, self.bytes_per_sector);
                        return rec;
                    }
                    None => return vec![0u8; self.mft_record_size as usize], // sparse
                }
            }
            run_start += run_len_bytes;
        }
        vec![0u8; self.mft_record_size as usize]
    }

    fn iter_attributes(record: &[u8], _record_size: u64) -> Vec<(u32, usize)> {
        // attrs_offset at bytes 20-21
        let mut attrs = Vec::new();
        if record.len() < 22 { return attrs; }
        let mut pos = u16::from_le_bytes([record[20], record[21]]) as usize;
        while pos + 4 <= record.len() {
            let attr_type = u32::from_le_bytes([
                record[pos], record[pos+1], record[pos+2], record[pos+3],
            ]);
            if attr_type == ATTR_END { break; }
            if pos + 8 > record.len() { break; }
            let attr_len = u32::from_le_bytes([
                record[pos+4], record[pos+5], record[pos+6], record[pos+7],
            ]) as usize;
            if attr_len == 0 { break; }
            attrs.push((attr_type, pos));
            pos += attr_len;
        }
        attrs
    }


    fn attr_content(&self, record: &[u8], pos: usize) -> Vec<u8> {
        if pos + 9 > record.len() { return Vec::new(); }
        let non_resident = record[pos + 8]; // 0 = resident, 1 = non-resident
        if non_resident == 0 {
            if pos + 24 > record.len() { return Vec::new(); }
            let content_off = u16::from_le_bytes([record[pos+20], record[pos+21]]) as usize;
            let content_len = u32::from_le_bytes([
                record[pos+16], record[pos+17], record[pos+18], record[pos+19],
            ]) as usize;
            let start = pos + content_off;
            let end   = start + content_len;
            if end > record.len() { return Vec::new(); }
            record[start..end].to_vec()
        } else {
            if pos + 64 > record.len() { return Vec::new(); }
            let runlist_off = u16::from_le_bytes([record[pos+32], record[pos+33]]) as usize;
            let real_size   = u64::from_le_bytes([
                record[pos+48], record[pos+49], record[pos+50], record[pos+51],
                record[pos+52], record[pos+53], record[pos+54], record[pos+55],
            ]);
            let attr_len = u32::from_le_bytes([
                record[pos+4], record[pos+5], record[pos+6], record[pos+7],
            ]) as usize;
            let rl_start = pos + runlist_off;
            let rl_end   = pos + attr_len;
            if rl_end > record.len() || rl_start >= rl_end { return Vec::new(); }
            let runs = decode_run_list(&record[rl_start..rl_end]);
            let mut out = Vec::with_capacity(real_size as usize);
            for (lcn_opt, len_clusters) in &runs {
                if out.len() as u64 >= real_size { break; }
                let run_bytes = len_clusters * self.cluster_size;
                match lcn_opt {
                    Some(lcn) => {
                        let remaining = real_size - out.len() as u64;
                        let to_read = run_bytes.min(remaining);
                        let mut data = self.read_bytes(self.lcn_to_byte(*lcn), to_read);
                        out.append(&mut data);
                    }
                    None => {
                        let remaining = real_size - out.len() as u64;
                        let to_fill = run_bytes.min(remaining) as usize;
                        out.extend(std::iter::repeat(0u8).take(to_fill));
                    }
                }
            }
            out
        }
    }

    fn get_data_runs(record: &[u8], record_size: u64) -> Vec<(Option<u64>, u64)> {
        let attrs = Self::iter_attributes(record, record_size);
        for (attr_type, pos) in &attrs {
            if *attr_type == ATTR_DATA {
                let non_resident = record[pos + 8];
                if non_resident == 1 {
                    let runlist_off = u16::from_le_bytes([record[pos+32], record[pos+33]]) as usize;
                    let attr_len = u32::from_le_bytes([
                        record[pos+4], record[pos+5], record[pos+6], record[pos+7],
                    ]) as usize;
                    let rl_start = pos + runlist_off;
                    let rl_end   = pos + attr_len;
                    if rl_end <= record.len() && rl_start < rl_end {
                        return decode_run_list(&record[rl_start..rl_end]);
                    }
                }
            }
        }
        Vec::new()
    }

    fn index_from_root(mut self) -> Self {
        self.directories.push(Directory {
            name: self.mounted_at.clone() + "/",
        });

        // Count MFT records from total MFT byte size
        let total_mft_bytes: u64 = self.mft_runs.iter()
            .map(|(_, len)| len * self.cluster_size)
            .sum();
        let total_records = total_mft_bytes / self.mft_record_size;

        for idx in 0..total_records {
            let record = self.read_mft_record(idx);

            // Verify "FILE" signature
            if record.len() < 4 || &record[0..4] != b"FILE" { continue; }

            let flags = u16::from_le_bytes([record[22], record[23]]);
            if flags & MFT_RECORD_IN_USE == 0 { continue; } // deleted

            let is_dir = flags & MFT_RECORD_IS_DIR != 0;

            // Parse $STANDARD_INFORMATION for timestamps
            let mut create_timestamp = 0i64;
            let mut last_modified_timestamp = 0i64;

            // Parse $FILE_NAME — we want the best namespace available
            let mut best_namespace: Option<u8> = None;
            let mut file_name = String::new();
            let mut parent_mft_idx = 5u64;

            let attrs = Self::iter_attributes(&record, self.mft_record_size);

            for (attr_type, pos) in &attrs {
                match *attr_type {
                    ATTR_STANDARD_INFORMATION => {
                        let data = self.attr_content(&record, *pos);
                        if data.len() >= 16 {
                            let created  = u64::from_le_bytes(data[0..8].try_into().unwrap());
                            let modified = u64::from_le_bytes(data[8..16].try_into().unwrap());
                            create_timestamp        = filetime_to_epoch(created);
                            last_modified_timestamp = filetime_to_epoch(modified);
                        }
                    }
                    ATTR_FILE_NAME => {
                        let data = self.attr_content(&record, *pos);
                        if data.len() < 66 { continue; }
                        let par = u64::from_le_bytes([
                            data[0],data[1],data[2],data[3],data[4],data[5],0,0,
                        ]);
                        let namespace   = data[65];
                        let name_length = data[64] as usize;
                        if data.len() < 66 + name_length * 2 { continue; }

                        let name_u16: Vec<u16> = (0..name_length)
                            .map(|i| u16::from_le_bytes([data[66 + i*2], data[67 + i*2]]))
                            .collect();
                        let name = String::from_utf16_lossy(&name_u16).to_string();

                        let priority = match namespace {
                            NAMESPACE_WIN32DOS => 3,
                            NAMESPACE_WIN32    => 2,
                            NAMESPACE_DOS      => 1,
                            NAMESPACE_POSIX    => 0,
                            _                  => 0,
                        };
                        let cur_priority = best_namespace.map(|n| match n {
                            NAMESPACE_WIN32DOS => 3,
                            NAMESPACE_WIN32    => 2,
                            NAMESPACE_DOS      => 1,
                            _                  => 0,
                        }).unwrap_or(255);

                        if best_namespace.is_none() || priority > cur_priority {
                            best_namespace  = Some(namespace);
                            file_name       = name;
                            parent_mft_idx  = par;
                        }
                    }
                    _ => {}
                }
            }

            if file_name.is_empty() { continue; }

            // Skip system metafiles ($MFT, $MFTMirr, etc.) — MFT indices 0-11
            // but keep index 5 ($. / root) which we handle as the mount point
            if idx < 12 && idx != MFT_RECORD_ROOT { continue; }
            if idx == MFT_RECORD_ROOT { continue; } // root is already pushed as directory[0]

            // Get file size from $DATA attribute (0 for dirs)
            let size = if is_dir {
                0u64
            } else {
                self.get_file_size(&record)
            };

            self.files.push(NtfsFile {
                name: file_name,
                mft_index: idx,
                parent_mft_index: parent_mft_idx,
                parent: 0,
                size,
                is_dir,
                create_timestamp,
                last_modified_timestamp,
            });
        }

        let _mft_to_file_pos: std::collections::HashMap<u64, usize> = self.files.iter()
            .enumerate()
            .map(|(i, f)| (f.mft_index, i))
            .collect();

        // Build directory list from dirs in self.files, and assign parent indices
        // We do a BFS from root outward so parents are always registered before children.
        // For simplicity (matching exFAT/ext4 style): push all dirs, then resolve.
        // Directory 0 is already the mount root.
        let mut mft_to_dir_idx: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
        mft_to_dir_idx.insert(MFT_RECORD_ROOT, 0);

        let dir_files: Vec<(usize, u64)> = self.files.iter().enumerate()
            .filter(|(_, f)| f.is_dir)
            .map(|(i, f)| (i, f.mft_index))
            .collect();

        for (file_pos, mft_idx) in &dir_files {
            let parent_mft = self.files[*file_pos].parent_mft_index;
            let parent_dir_idx = *mft_to_dir_idx.get(&parent_mft).unwrap_or(&0);
            let parent_name = self.directories[parent_dir_idx as usize].name.clone();
            let dir_name = parent_name + &self.files[*file_pos].name + "/";

            // Check ignored dirs
            if self.ignored_dirs.iter().any(|ig| dir_name.starts_with(ig)) { continue; }

            self.directories.push(Directory { name: dir_name });
            let new_dir_idx = self.directories.len() as u32 - 1;
            mft_to_dir_idx.insert(*mft_idx, new_dir_idx);
            self.files[*file_pos].parent = new_dir_idx;
        }

        for f in self.files.iter_mut() {
            if !f.is_dir {
                f.parent = *mft_to_dir_idx.get(&f.parent_mft_index).unwrap_or(&0);
            }
        }

        // Filter ignored files
        let dirs = &self.directories;
        let ignored = &self.ignored_dirs;
        self.files.retain(|f| {
            let parent_name = &dirs[f.parent as usize].name;
            let full = parent_name.clone() + &f.name;
            !ignored.iter().any(|ig| full.starts_with(ig))
        });

        self
    }

    fn get_file_size(&self, record: &[u8]) -> u64 {
        let attrs = Self::iter_attributes(record, self.mft_record_size);
        for (attr_type, pos) in &attrs {
            if *attr_type == ATTR_DATA {
                let non_resident = record[pos + 8];
                if non_resident == 0 {
                    // Resident: content_length at pos+16
                    return u32::from_le_bytes([
                        record[pos+16], record[pos+17], record[pos+18], record[pos+19],
                    ]) as u64;
                } else {
                    // Non-resident: real size at pos+48
                    if pos + 56 <= record.len() {
                        return u64::from_le_bytes([
                            record[pos+48], record[pos+49], record[pos+50], record[pos+51],
                            record[pos+52], record[pos+53], record[pos+54], record[pos+55],
                        ]);
                    }
                }
            }
        }
        0
    }
}

#[derive(Debug, Default, Clone)]
struct NtfsFile {
    name:                   String,
    mft_index:              u64,
    parent_mft_index:       u64,
    parent:                 u32,
    size:                   u64,
    is_dir:                 bool,
    create_timestamp:       i64,
    last_modified_timestamp: i64,
}

fn from_ntfs_file_to_file(f: &NtfsFile) -> File {
    File {
        name:                   f.name.clone(),
        parent:                 f.parent,
        size:                   f.size,
        is_dir:                 f.is_dir,
        create_timestamp:       f.create_timestamp,
        last_modified_timestamp: f.last_modified_timestamp,
    }
}
pub fn is_drive_valid(drive: String) -> bool{
    let file = fs::File::open(drive);
    if file.is_err(){return false}
    let file = file.unwrap();
    let mut vbr = vec![0u8; 512];
    file.read_at(&mut vbr, 0).unwrap();

    // magic bytes "NTFS    "
    if &vbr[3..11] == b"NTFS    "{
        true
    }else{
        false
    }
}
pub fn index(
    drive: String,
    mounted_at: String,
    ignored_dirs: Vec<String>,
) -> Result<(Vec<File>, Vec<Directory>), u32> {
    let drive = NtfsDrive::new(drive, mounted_at, ignored_dirs);
    if drive.is_err(){
        return Err(drive.err().unwrap());
    }
    let drive = drive.unwrap().index_from_root();

    let files = drive.files.iter()
        .map(|f| from_ntfs_file_to_file(f))
        .collect();

    Ok((files, drive.directories))
}