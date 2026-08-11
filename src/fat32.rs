use std::fs;
use std::os::unix::fs::FileExt;
use std::io::{Read};
use crate::{Directory, File};

// The following code decodes the FAT32 filesystem following the Microsoft
// FAT specification (Microsoft Extensible Firmware Initiative FAT32 File
// System Specification)
use chrono::{FixedOffset, NaiveDate,TimeZone};
fn to_epoch(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32, ms: u32, offset_secs: i32) -> i64 {
    let offset = FixedOffset::east_opt(offset_secs).expect("Invalid offset");
    let naive_date = NaiveDate::from_ymd_opt(year, month, day).expect("Invalid date");
    let naive_dt = naive_date.and_hms_milli_opt(hour, minute, second, ms).expect("Invalid time");
    let local_result = offset.from_local_datetime(&naive_dt);
    let dt = local_result.unwrap();  // Or use .single().unwrap() for safety
    dt.timestamp()  // Now works on DateTime<FixedOffset>
}

// FAT date/time fields are packed into two u16 words (plus an optional
// tenth-of-a-second byte for creation time). There is no timezone stored,
// unlike exFAT, so offset_secs is always 0.
fn bytes_to_time(t1: u8, t2: u8, d1: u8, d2: u8, tenth: u8) -> i64{
    let time = u16::from_le_bytes([t1, t2]);
    let date = u16::from_le_bytes([d1, d2]);

    let mut second = ((time & 0b0000000000011111) as u32) * 2;
    let minute = ((time & 0b0000011111100000) >> 5) as u32;
    let hour = ((time & 0b1111100000000000) >> 11) as u32;

    let day = (date & 0b0000000000011111) as u32;
    let month = ((date & 0b0000000111100000) >> 5) as u32;
    let year = 1980 + ((date & 0b1111111000000000) >> 9) as u32;

    let mut ms = 10 * (tenth as u32 % 100);
    if ms > 999{
        ms -= 1000;
        second += 1;
    }

    if second > 60{return 0}
    if minute > 59{return 0}
    if hour > 23{return 0}
    if day < 1 || day > 31{return 0}
    if month < 1 || month > 12{return 0}
    if year < 1980 || year > 2107{return 0}

    to_epoch(
        year as i32, month as u32, day as u32,
        hour as u32, minute as u32, second as u32,
        ms as u32, 0
    )
}

struct Fat32Drive{
    file: fs::File,
    directories: Vec<Directory>,
    volume_label: String,
    mounted_at: String,
    bytes_per_sector: u64,
    sectors_per_cluster: u64,
    cluster_size: u64,
    cluster_byte_heap_offset: u64,
    root_dir_cluster: u64,
    files: Vec<Fat32File>,
    fat_table: Vec<u32>,
    ignored_dirs: Vec<String>
}
impl Fat32Drive{
    fn new(device: String, mounted_at: String, ignored_dirs: Vec<String>)-> Result<Self, u32>{
        let file = fs::File::open(device);
        if file.is_err(){
            return Err(1);
        }
        let mut file = file.unwrap();
        let mut buffer = vec![0u8; 512];
        let _bytes_read = file.read(&mut buffer).unwrap();
        if vec![70,65,84,51,50,32,32,32] != buffer[82..90]{
            return Err(100);
        }
        // assert_eq!(vec![70,65,84,51,50,32,32,32],buffer[82..90]); //"FAT32   " flag
        assert_eq!([0x55, 0xAA], [buffer[510], buffer[511]], "Invalid boot signature"); //BootSignature flag
        // Bytes per sector
        let bytes_per_sector = u16::from_le_bytes([buffer[11], buffer[12]]) as u64;
        // Sectors per cluster
        let sectors_per_cluster = buffer[13] as u64;
        // Reserved area / FAT layout
        let reserved_sector_count = u16::from_le_bytes([buffer[14], buffer[15]]) as u64;
        let num_fats = buffer[16] as u64;
        let fat_size_32 = u32::from_le_bytes([buffer[36], buffer[37], buffer[38], buffer[39]]) as u64;
        // Root dir location (cluster chain, unlike FAT12/16)
        let root_dir_cluster = u32::from_le_bytes([buffer[44], buffer[45], buffer[46], buffer[47]]) as u64;

        let cluster_size = bytes_per_sector*sectors_per_cluster;

        // Where files start
        let first_data_sector = reserved_sector_count + num_fats*fat_size_32;
        let cluster_byte_heap_offset = first_data_sector*bytes_per_sector;

        //Fat table
        let fat_table_offset = reserved_sector_count;
        let mut fat_table = Vec::new();
        let mut b = vec![0_u8; fat_size_32 as usize * bytes_per_sector as usize];
        file.read_at(&mut b, fat_table_offset as u64*bytes_per_sector as u64).unwrap();
        for i in 0..fat_size_32*bytes_per_sector as u64/4{
            fat_table.push(u32::from_le_bytes([b[(i*4) as usize], b[(i*4)as usize+1],
                                            b[(i*4)as usize+2], b[(i*4)as usize+3]]));
        }
        let directories = Vec::new();
        Ok(Fat32Drive {directories, ignored_dirs,mounted_at,fat_table,file, volume_label: String::new(), bytes_per_sector, sectors_per_cluster, cluster_size, cluster_byte_heap_offset, root_dir_cluster, files: Vec::new()})
    }
    fn cluster_to_byte(self: &Self, cluster: u64)->u64{
        (cluster-2)*self.bytes_per_sector*self.sectors_per_cluster+self.cluster_byte_heap_offset
    }
    fn read_bytes(self: &Self, from: u64, size: u64) -> Vec<u8>{
        let mut b = vec![0_u8; size as usize];
        self.file.read_at(&mut b, from).unwrap();
        return b;
    }
    fn index_from_root(mut self: Self) -> Self{
        self.directories.push(Directory { name: self.mounted_at.clone() + "/" });
        let mut clusters = vec![self.root_dir_cluster];
        let mut next_cluster = self.find_next_in_fat(self.root_dir_cluster as u32) as u64;
        while next_cluster != 0 && next_cluster < 0x0FFFFFF8{
            clusters.push(next_cluster);
            next_cluster = self.find_next_in_fat(next_cluster as u32) as u64;
        }
        let mut bytes = Vec::new();
        for c in 0..clusters.len(){
            bytes.append(&mut self.read_bytes(self.cluster_to_byte(clusters[c] as u64), self.cluster_size));
        }
        let mut found_eod = false; //end_of_directory
        let mut lfn_buf: Vec<(u8, Vec<u16>)> = Vec::new();
        let mut i = 0;
        let size = bytes.len() as u64;
        while i<size{
            match bytes[i as usize]{
                0x00 => {found_eod = true}
                0xE5 => {lfn_buf.clear()} // deleted entry
                _ => {
                    let i = i.clone() as usize;
                    let attr = bytes[i+11];
                    if attr == 0x0F{
                        // long file name entry
                        let order = bytes[i] & 0x1F;
                        let mut chars = Vec::new();
                        for off in [1,3,5,7,9,14,16,18,20,22,24,28,30]{
                            let c = u16::from_le_bytes([bytes[i+off], bytes[i+off+1]]);
                            if c == 0x0000 || c == 0xFFFF{break}
                            chars.push(c);
                        }
                        lfn_buf.push((order, chars));
                    }else if attr & 0x08 != 0{
                        // volume label entry
                        let mut name = Vec::new();
                        for j in 0..11{
                            name.push(bytes[i+j]);
                        }
                        self.volume_label = String::from_utf8_lossy(&name).trim().to_string();
                        lfn_buf.clear();
                    }else if bytes[i] == b'.'{
                        // "." or ".." entries
                        lfn_buf.clear();
                    }else{
                        let is_a_dir = attr & 0b00010000;
                        let created_t = bytes_to_time(bytes[i+14], bytes[i+15], bytes[i+16], bytes[i+17], bytes[i+13]);
                        let modified_t = bytes_to_time(bytes[i+22], bytes[i+23], bytes[i+24], bytes[i+25], 0);
                        let first_cluster = ((u16::from_le_bytes([bytes[i+20],bytes[i+21]]) as u32)<<16)
                            | u16::from_le_bytes([bytes[i+26],bytes[i+27]]) as u32;
                        let size = u32::from_le_bytes([bytes[i+28],bytes[i+29],bytes[i+30],bytes[i+31]]) as u64;

                        let name;
                        if !lfn_buf.is_empty(){
                            lfn_buf.sort_by_key(|(order, _)| *order);
                            let mut utf16 = Vec::new();
                            for (_, chars) in &lfn_buf{
                                utf16.extend(chars);
                            }
                            name = String::from_utf16(&utf16).unwrap_or_default();
                        }else{
                            name = String::from_utf8_lossy(&bytes[i..i+11]).trim().to_string();
                        }
                        lfn_buf.clear();

                        let mut full_name = self.mounted_at.clone() + "/" + &name;
                        let mut is_dir = false;
                        if is_a_dir == 0b10000{
                            is_dir = true;
                            full_name += "/";
                        }
                        let mut to_ignore = false;
                        for i in self.ignored_dirs.clone(){
                            if full_name.starts_with(&i){
                                to_ignore = true;
                            }
                        }
                        if !name.is_empty() && !to_ignore{
                            self.files.push(
                                Fat32File{
                                    parent: 0,
                                    first_cluster,
                                    is_dir,
                                    name,
                                    size,
                                    create_timestamp:created_t,
                                    last_modified_timestamp:modified_t
                                }
                            );
                        }
                    }
                }
            }

            if i+32>=size && !found_eod{

            }
            i += 32;
        }
        self
    }
    fn index(self: &mut Self, directory: &Fat32File, parent: u32){
        let mut clusters = vec![directory.first_cluster as u64];
        let mut next_cluster = self.find_next_in_fat(directory.first_cluster);
        while next_cluster != 0 && (next_cluster as u64) < 0x0FFFFFF8{
            clusters.push(next_cluster as u64);
            next_cluster = self.find_next_in_fat(next_cluster);
        }
        let mut bytes = Vec::new();
        for c in 0..clusters.len(){
            bytes.append(&mut self.read_bytes(self.cluster_to_byte(clusters[c] as u64), self.cluster_size));
        }
        let mut new_files = Vec::new();
        let mut found_eod = false; //end_of_directory
        let mut lfn_buf: Vec<(u8, Vec<u16>)> = Vec::new();
        let mut i = 0;
        let dir_size = bytes.len() as u64;
        while i<dir_size{
            match bytes[i as usize]{
                0x00 => {found_eod = true}
                0xE5 => {lfn_buf.clear()}
                _ => {
                    let i = i.clone() as usize;
                    let attr = bytes[i+11];
                    if attr == 0x0F{
                        let order = bytes[i] & 0x1F;
                        let mut chars = Vec::new();
                        for off in [1,3,5,7,9,14,16,18,20,22,24,28,30]{
                            let c = u16::from_le_bytes([bytes[i+off], bytes[i+off+1]]);
                            if c == 0x0000 || c == 0xFFFF{break}
                            chars.push(c);
                        }
                        lfn_buf.push((order, chars));
                    }else if attr & 0x08 != 0{
                        lfn_buf.clear();
                    }else if bytes[i] == b'.'{
                        lfn_buf.clear();
                    }else{
                        let is_a_dir = attr & 0b00010000;
                        let created_t = bytes_to_time(bytes[i+14], bytes[i+15], bytes[i+16], bytes[i+17], bytes[i+13]);
                        let modified_t = bytes_to_time(bytes[i+22], bytes[i+23], bytes[i+24], bytes[i+25], 0);
                        let first_cluster = ((u16::from_le_bytes([bytes[i+20],bytes[i+21]]) as u32)<<16)
                            | u16::from_le_bytes([bytes[i+26],bytes[i+27]]) as u32;
                        let size = u32::from_le_bytes([bytes[i+28],bytes[i+29],bytes[i+30],bytes[i+31]]) as u64;

                        let name;
                        if !lfn_buf.is_empty(){
                            lfn_buf.sort_by_key(|(order, _)| *order);
                            let mut utf16 = Vec::new();
                            for (_, chars) in &lfn_buf{
                                utf16.extend(chars);
                            }
                            name = String::from_utf16(&utf16).unwrap_or_default();
                        }else{
                            name = String::from_utf8_lossy(&bytes[i..i+11]).trim().to_string();
                        }
                        lfn_buf.clear();

                        let mut full_name = self.directories[directory.parent as usize].name.clone() + &directory.name + &name;
                        let mut is_dir = false;
                        if is_a_dir == 0b10000{
                            is_dir = true;
                            full_name += "/";
                        }
                        let mut to_ignore = false;
                        for i in self.ignored_dirs.clone(){
                            if full_name.starts_with(&i){
                                to_ignore = true;
                            }
                        }
                        if to_ignore{break;}
                        if !name.is_empty(){
                            let file = Fat32File{parent, first_cluster,is_dir, name,size,create_timestamp:created_t,last_modified_timestamp:modified_t};
                            self.files.push(file.clone());
                            new_files.push(file);
                        }
                    }
                }
            }
            if found_eod{
                break;
            }
            if i+32>=dir_size && !found_eod{
            }
            i += 32;
        }
        for file in new_files{
            if file.is_dir{
                let name = self.directories[file.parent as usize].name.clone() + &file.name + "/";
                self.directories.push(Directory{name});
                self.index(&file.clone(), self.directories.len() as u32 - 1);
            }
        }

    }
    fn find_next_in_fat(self: &Self, val: u32) -> u32{
        return self.fat_table[val as usize] & 0x0FFFFFFF;
    }
}
/// A file, timestamps use unix epoch
#[derive(Debug, Default, Clone)]
struct Fat32File{
    name: String,
    parent: u32,
    size: u64,
    is_dir: bool,
    create_timestamp: i64,
    last_modified_timestamp: i64,
    first_cluster: u32,
}
fn from_fat32_files_to_files(f: &Fat32File)->File{
    File{
        name:f.name.clone(),
        parent:f.parent,
        size:f.size,
        is_dir:f.is_dir,
        create_timestamp:f.create_timestamp,
        last_modified_timestamp: f.last_modified_timestamp
    }
}
pub fn is_drive_valid(drive: String) -> bool{
    let file = fs::File::open(drive);
    if file.is_err(){return false}
    let mut file = file.unwrap();
    let mut buffer = vec![0u8; 512];
    let _bytes_read = file.read(&mut buffer).unwrap();
    if vec![70,65,84,51,50,32,32,32] == buffer[82..90]{
        true
    }else{
        false
    }
}
pub fn index(drive: String, mounted_at: String, ignored_dirs: Vec<String>) -> Result<(Vec<File>, Vec<Directory>), u32> {

    let drive = Fat32Drive::new(drive, mounted_at, ignored_dirs);
    if drive.is_err(){
        return Err(drive.err().unwrap());
    }
    let mut drive = drive.unwrap().index_from_root();
    for i in 0..drive.files.len(){
        if drive.files[i].is_dir{
            let name = drive.directories[drive.files[i].parent as usize].name.clone() + &drive.files[i].name + "/";
            drive.directories.push(Directory{name});
            drive.index(&drive.files[i].clone(), drive.directories.len() as u32 - 1);
        }
    }
    let mut output = Vec::new();
    for f in drive.files{
        output.push(from_fat32_files_to_files(&f));
    }
    Ok((output,drive.directories))
}