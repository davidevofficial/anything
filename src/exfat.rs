use std::fs;
use std::os::unix::fs::FileExt;
use std::io::{Read};
use crate::File;

// The following code decodes the exFAT filesystem following the exfat spec
// https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification
use chrono::{FixedOffset, NaiveDate,TimeZone};
fn to_epoch(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32, ms: u32, offset_secs: i32) -> i64 {
    let offset = FixedOffset::east_opt(offset_secs).expect("Invalid offset");
    let naive_date = NaiveDate::from_ymd_opt(year, month, day).expect("Invalid date");
    let naive_dt = naive_date.and_hms_milli_opt(hour, minute, second, ms).expect("Invalid time");
    let local_result = offset.from_local_datetime(&naive_dt);
    let dt = local_result.unwrap();  // Or use .single().unwrap() for safety
    dt.timestamp()  // Now works on DateTime<FixedOffset>
}

const QUARTER: i32 = 900;
fn bytes_to_time(b1: u8, b2: u8, b3: u8, b4: u8, b_ms: u8, b_tz: u8)->i64{
    let mut year = 0;
    let mut month = 0;
    let mut day = 0;
    let mut hour = 0;
    let mut minute = 0;
    let mut second = 0;
    let mut ms = 0;
    let mut offset_secs = 0;


    0
    // to_epoch(year, month, day, hour, minute, second, ms, offset_secs)


}
struct ExFATDrive{
    file: fs::File,
    volume_label: String,
    mounted_at: String,
    bytes_per_sector: u64,
    sectors_per_cluster: u64,
    cluster_size: u64,
    cluster_byte_heap_offset: u64,
    root_dir_cluster: u64,
    files: Vec<ExFatFile>,
    fat_table: Vec<u32>,
    ignored_dirs: Vec<String>
}
impl ExFATDrive{
    fn new(device: String, mounted_at: String, ignored_dirs: Vec<String>)-> Self{
        let mut file = fs::File::open(device).unwrap();
        let mut buffer = vec![0u8; 512];
        let _bytes_read = file.read(&mut buffer).unwrap();
        assert_eq!(vec![69,88,70,65,84,32,32,32],buffer[3..11]); //ExFat flag
        assert_eq!([0x55, 0xAA], [buffer[510], buffer[511]], "Invalid boot signature"); //BootSignature flag
        // Bytes per sector
        let bytes_per_sector_shift = buffer[108] as u64;  // 2^(this number) = sector size
        let bytes_per_sector = 1u64 << bytes_per_sector_shift;
        // Sector per cluster
        let sectors_per_cluster_shift = buffer[109] as u64;
        let sectors_per_cluster = 1u64 << sectors_per_cluster_shift;
        // Where files start
        let cluster_heap_offset_sectors = u32::from_le_bytes([buffer[88], buffer[89], buffer[90], buffer[91]]) as u64;
        let cluster_byte_heap_offset = cluster_heap_offset_sectors*bytes_per_sector;
        // Root dir location
        let root_dir_cluster = u32::from_le_bytes([buffer[96], buffer[97], buffer[98], buffer[99]]) as u64;

        let cluster_size = bytes_per_sector*sectors_per_cluster;

        //Fat table
        let fat_table_length = u32::from_le_bytes([buffer[84], buffer[85], buffer[86], buffer[87]]);
        let fat_table_offset = u32::from_le_bytes([buffer[80], buffer[81], buffer[82], buffer[83]]);
        let mut fat_table = Vec::new();
        let mut b = vec![0_u8; fat_table_length as usize * bytes_per_sector as usize];
        file.read_at(&mut b, fat_table_offset as u64*bytes_per_sector as u64).unwrap();
        for i in 0..fat_table_length*bytes_per_sector as u32/4{
            fat_table.push(u32::from_le_bytes([b[(i*4) as usize], b[(i*4)as usize+1],
                                            b[(i*4)as usize+2], b[(i*4)as usize+3]]));
        }

        ExFATDrive {ignored_dirs,mounted_at,fat_table,file, volume_label: String::new(), bytes_per_sector, sectors_per_cluster, cluster_size, cluster_byte_heap_offset, root_dir_cluster, files: Vec::new()}
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
        // dbg!(self.cluster_to_byte(self.root_dir_cluster));
        let mut clusters = vec![self.root_dir_cluster];
        let mut next_cluster = self.root_dir_cluster.clone();
        while self.find_next_in_fat(next_cluster as u32) != 0 && self.find_next_in_fat(next_cluster as u32) != u32::MAX{
            clusters.push(next_cluster);
            next_cluster = self.find_next_in_fat(next_cluster as u32) as u64;
        }
        let mut bytes = Vec::new();
        for c in 0..clusters.len(){
            bytes.append(&mut self.read_bytes(self.cluster_to_byte(clusters[c] as u64), self.cluster_size));
        }
        let mut found_eod = false; //end_of_directory
        let mut i = 0;
        let size = self.cluster_size*clusters.len() as u64;
        while i<size{

            match bytes[i as usize]{
                0x00 => {found_eod = true}
                0x83 => {
                    let size = bytes[(i+1) as usize];
                    let mut volume_label = Vec::new();
                    for j in 0..size{
                        volume_label.push(u16::from_le_bytes([bytes[(i+(j*2) as u64+2) as usize],bytes[(i+(j*2) as u64+3) as usize]]))
                    }
                    self.volume_label = String::from_utf16(&volume_label).unwrap();
                }
                0x85 => {
                    let i = i.clone() as usize;
                    let _secondary_count = bytes[i+1];
                    let attr_1 = bytes[i+4];
                    let is_a_dir = attr_1 & 0b00010000;
                    let created_t = bytes_to_time(bytes[i+8], bytes[i+9], bytes[i+10],
                        bytes[i+11], bytes[i+20], bytes[i+23]);
                    let modified_t = bytes_to_time(bytes[i+12], bytes[i+13], bytes[i+14],
                        bytes[i+15],bytes[i+21], bytes[i+24]);
                    let name_length = bytes[i+35];
                    let first_cluster = u32::from_le_bytes([bytes[i+52],bytes[i+53],bytes[i+54],bytes[i+55]]);
                    let size = u64::from_le_bytes([bytes[i+56],bytes[i+57],bytes[i+58],bytes[i+59],
                                                        bytes[i+60],bytes[i+61],bytes[i+62],bytes[i+63]]);

                    let mut name = Vec::new();
                    let mut k = 64_usize+i;
                    for _ in 0..name_length{
                        if k % 32 == 0{
                            k += 2;
                        }
                        name.push(u16::from_le_bytes([bytes[k], bytes[k+1]]));
                        k+=2
                    }
                    let name = String::from_utf16(&name).unwrap();
                    let mut full_name = self.mounted_at.clone() + "/" + &name;
                    let mut is_dir = false;
                    if is_a_dir == 0b10000{
                        is_dir = true;
                        full_name += "/";
                    }
                    let secondary_flags = bytes[i+33];
                    let contigous = secondary_flags & 0b00000010;
                    let contigous = if contigous == 2{true}else{false};
                    let mut to_ignore = false;
                    for i in self.ignored_dirs.clone(){
                        if full_name.starts_with(&i){
                            to_ignore = true;
                        }
                    }
                    if !to_ignore{
                        self.files.push(
                            ExFatFile{
                                full_name,
                                contigous,
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
                _ => {}
            }

            if i+32>=size && !found_eod{

            }
            i += 32;
        }
        self
    }
    fn index(self: &mut Self, directory: &ExFatFile){
        let cluster_n = directory.size / self.cluster_size;
        let mut clusters = Vec::new();
        clusters.push(directory.first_cluster);
        if !directory.contigous{
            for i in 1..cluster_n{
                let mut next_cluster = directory.first_cluster;
                for _ in 0..i{
                    next_cluster = self.find_next_in_fat(next_cluster);
                }
                clusters.push(next_cluster);
            }
        }else{
            for i in 1..cluster_n{
                clusters.push(directory.first_cluster+i as u32);
            }
        }
        let mut bytes = Vec::new();
        for c in 0..clusters.len(){
            bytes.append(&mut self.read_bytes(self.cluster_to_byte(clusters[c] as u64), self.cluster_size));
        }
        assert_eq!(bytes.len(), directory.size as usize);
        let mut new_files = Vec::new();
        let mut found_eod = false; //end_of_directory
        let mut i = 0;
        while i<directory.size{
            match bytes[i as usize]{
                0x00 => {found_eod = true}
                0x85 => {
                    let i = i.clone() as usize;
                    let _secondary_count = bytes[i+1];
                    let attr_1 = bytes[i+4];
                    let is_a_dir = attr_1 & 0b00010000;
                    let created_t = bytes_to_time(bytes[i+8], bytes[i+9], bytes[i+10],
                        bytes[i+11], bytes[i+20], bytes[i+23]);
                    let modified_t = bytes_to_time(bytes[i+12], bytes[i+13], bytes[i+14],
                        bytes[i+15],bytes[i+21], bytes[i+24]);
                    let name_length = bytes[i+35];
                    let first_cluster = u32::from_le_bytes([bytes[i+52],bytes[i+53],bytes[i+54],bytes[i+55]]);
                    let size = u64::from_le_bytes([bytes[i+56],bytes[i+57],bytes[i+58],bytes[i+59],
                                                        bytes[i+60],bytes[i+61],bytes[i+62],bytes[i+63]]);

                    let mut name2 = Vec::new();
                    let mut k = 64_usize+i;
                    for _ in 0..name_length{
                        if k % 32 == 0{
                            k += 2;
                        }
                        name2.push(u16::from_le_bytes([bytes[k], bytes[k+1]]));
                        k+=2
                    }
                    let name = String::from_utf16(&name2);
                    let name = name.unwrap();
                    let mut full_name = directory.full_name.clone() + &name;
                    let mut is_dir = false;
                    if is_a_dir == 0b10000{
                        is_dir = true;
                        full_name += "/";
                    }
                    let secondary_flags = bytes[i+33];
                    let contigous = secondary_flags & 0b00000010;
                    let contigous = if contigous == 2{true}else{false};
                    let mut to_ignore = false;
                    for i in self.ignored_dirs.clone(){
                        if full_name.starts_with(&i){
                            to_ignore = true;
                        }
                    }
                    if to_ignore{break;}
                    let file = ExFatFile{full_name, contigous,first_cluster,is_dir, name,size,create_timestamp:created_t,last_modified_timestamp:modified_t};
                    self.files.push(file.clone());
                    new_files.push(file);
                }
                _ => {}

            }
            if found_eod{
                break;
            }
            if i+32>=directory.size && !found_eod{
            }
            i += 32;
        }
        for file in new_files{
            if file.is_dir{
                self.index(&file.clone());
            }
        }

    }
    fn find_next_in_fat(self: &Self, val: u32) -> u32{
        return self.fat_table[val as usize];
    }
}
/// A file, timestamps use unix epoch
#[derive(Debug, Default, Clone)]
struct ExFatFile{
    name: String,
    full_name: String,
    size: u64,
    is_dir: bool,
    create_timestamp: i64,
    last_modified_timestamp: i64,
    first_cluster: u32,
    contigous: bool,
}
fn from_exfat_files_to_files(f: &ExFatFile)->File{
    File{
        name:f.name.clone(),
        full_name:f.full_name.clone(),
        size:f.size,
        is_dir:f.is_dir,
        create_timestamp:f.create_timestamp,
        last_modified_timestamp: f.last_modified_timestamp
    }
}

pub fn index(drive: String, mounted_at: String, ignored_dirs: Vec<String>) -> Vec<File> {
    let mut drive = ExFATDrive::new(drive, mounted_at, ignored_dirs).index_from_root();
    for i in 0..drive.files.len(){
        if drive.files[i].is_dir{
            drive.index(&drive.files[i].clone());
        }
    }
    let mut output = Vec::new();
    for f in drive.files{
        output.push(from_exfat_files_to_files(&f));
    }
    output
}