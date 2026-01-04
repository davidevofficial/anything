mod exfat;
mod frontend;
use chrono;
const SAVE_DRIVES_PATH: &str = "./settings/drives.txt";
const SAVE_SETTINGS_PATH: &str = "./settings/settings.txt";
const SAVE_CACHE_PATH: &str = "./settings/cache.txt";

pub fn size_to_pretty_string(size: u64) -> String{
    if size < 1024{
        return size.to_string() + "B";
    }
    if size < 1048576{
        return format!("{:.2}KiB", size as f64 / 1024.0);
    }
    if size < 1073741824{
        return format!("{:.2}MiB", size as f64 / 1048576.0);
    }
    if size < 1099511627776{
        return format!("{:.2}GiB", size as f64 / 1073741824.0);
    }else{
        return format!("{:.2}TiB", size as f64 / 1099511627776.0);
    }
}
pub fn timestamp_to_string(timestamp_ms: i64)-> String{
    let secs = (timestamp_ms / 1000);
    let nanos = ((timestamp_ms % 1000) * 1_000_000) as u32; // ms → ns

    let naive =chrono::DateTime::from_timestamp(secs, nanos).expect("Invalid Time");
    naive.format("%d/%m/%Y %H:%M:%S").to_string()
}
#[derive(Debug, Default, Clone)]
pub struct File{
    name: String,
    full_name: String,
    size: u64,
    is_dir: bool,
    create_timestamp: i64,
    last_modified_timestamp: i64,
}
#[derive(Debug, Default, Clone)]
pub struct Drive{
    fs: SupportedFilesystems,
    drive: String,
    mounted_at: String,
    ignored_dirs: Vec<String>
}
fn string_to_fs(string: &str) -> SupportedFilesystems{
    match string{
        "Exfat" => {SupportedFilesystems::Exfat}
        _ => {SupportedFilesystems::default()}
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum SupportedFilesystems{
    #[default]
    Exfat,
    // Soon ext4
}
#[derive(Debug, Default, Clone)]
pub struct Settings{
    /// file, path, size, date modified, date created
    columns: Vec<u16>,
    sort_in_use: Sort,
    index_on_startup: bool,
    index_every_minutes: u32,
    instant_search: bool,
    journal: bool,
    ignore_case: bool,
    search_full_path: bool
}
fn string_to_sort(string: &str) -> Sort{
    match string{
        "DateCreatedAscending" => {Sort::DateCreatedAscending}
        "DateCreatedDescending" => {Sort::DateCreatedDescending}
        "DateModifiedAscending" => {Sort::DateModifiedAscending}
        "DateModifiedDescending" => {Sort::DateModifiedDescending}
        "SizeAscending" => {Sort::SizeAscending}
        "SizeDescending" => {Sort::SizeDescending}
        "PathAscending" => {Sort::PathAscending}
        "PathDescending" => {Sort::PathDescending}
        "FileAscending" => {Sort::FileAscending}
        "FileDescending" => {Sort::FileDescending}
        _ => {Sort::default()}
    }
}
#[derive(Debug, Default, Clone, PartialEq)]
pub enum Sort{
    #[default]
    DateCreatedAscending,
    DateCreatedDescending,
    DateModifiedAscending,
    DateModifiedDescending,
    SizeAscending,
    SizeDescending,
    PathAscending,
    PathDescending,
    FileAscending,
    FileDescending
}
pub fn get_devices()->Vec<Drive>{
    let lsblk = std::process::Command::new("lsblk")
        .args(&["-l", "-n", "-o", "NAME,MOUNTPOINT"])
        .output()
        .expect("lsblk failed");
    let mut drives = Vec::new();
    let lines = lines_from_bytes(lsblk.stdout);
    for i in 0..lines.len(){
        if lines[i].contains(&b'/'){
            if lines[i][0] == b's' && lines[i][1] == b'd' || lines[i][0] == b'n'{
                let mut space = 0;
                for j in 0..lines[i].len(){
                    if lines[i][j] == b' '{space=j;break;}
                }
                let drive = &lines[i][0..space];
                let mut slash = 0;
                for j in 0..lines[i].len(){
                    if lines[i][j] == b'/'{slash=j;break;}
                }
                let mounted_at = &lines[i][slash..lines[i].len()-1];
                let mut drive = drive.to_vec();
                let mut dev = b"/dev/".to_vec();
                dev.append(&mut drive);
                let drive = String::from_utf8(dev).unwrap();
                let mounted_at = String::from_utf8(mounted_at.to_vec()).unwrap();
                drives.push(Drive{fs: SupportedFilesystems::default(),drive,mounted_at,ignored_dirs:vec![]});
            }
        }
    }
    drives
}
pub fn lines_from_bytes(mut data: Vec<u8>) -> Vec<Vec<u8>> {
    let mut lines = Vec::new();

    while let Some(pos) = data[0..].iter().position(|&b| b == b'\n') {
        let end = pos;
        lines.push(data.drain(0..=end).collect());
        // No need for start = end + 1; drain adjusts remaining data
    }

    // Last line
    if !data.is_empty() {
        lines.push(data.drain(..).collect());
    }

    lines
}

fn main()  {

    // assert_eq!(timestamp_to_string(910390), String::from("01/01/1970 00:15:10"));
    // let mut files = exfat::index(String::from("/dev/sdc1"), String::from("/media/1"), vec![
    //     // String::from("/media/1/music"),
    //     // String::from("/media/1/.Trash-1000"),
    //     // String::from("/media/1/data"),
    // ]);

    // //sort ascending
    // files.sort_by(|a,b| a.size.cmp(&b.size));
    // //sort descending
    // files.sort_by(|b,a| a.size.cmp(&b.size));

    let _ = frontend::start_frontend();
}

use std::io::{BufRead, BufWriter, Write};
pub fn save_drives(drives: Vec<Drive>){
    let file = std::fs::OpenOptions::new().write(true).truncate(true).open(SAVE_DRIVES_PATH).expect("NO {SAVE_DRIVES_PATH} found");
    let mut writer = BufWriter::new(file);
    // Write new lines, overwriting everything
    for drive in drives{
        let mut s = String::from("[");
        for dir in 0..drive.ignored_dirs.len(){
            if dir == drive.ignored_dirs.len()-1{
                s = format!("{s}{}",drive.ignored_dirs[dir]);
            }else{
                s = format!("{s}{}, ",drive.ignored_dirs[dir]);
            }
        }
        s = format!("{s}]");
        writeln!(writer, "{} {} {:?} {}",
            drive.drive, drive.mounted_at, drive.fs, s).unwrap();

    }
    writer.flush().unwrap();
}
pub fn load_drives() -> Vec<Drive>{
    let mut output = Vec::new();
    let file = std::fs::File::open(SAVE_DRIVES_PATH).expect("NO {SAVE_DRIVES_PATH} found");
    let reader = std::io::BufReader::new(file);
    for line in reader.lines(){
        let line = line.unwrap();
        let mut drive = String::new();
        let mut mounted_at = String::new();
        let mut fs = SupportedFilesystems::Exfat;
        let mut ignored_dirs = Vec::new();
        let mut i = 0;
        for attr in line.split(' '){
            match i{
                0=>{drive=attr.to_string()}
                1=>{mounted_at=attr.to_string()}
                2=>{fs=string_to_fs(attr)}
                _ =>{}
            }
            i+= 1;
        }
        let mut idx = 0;
        for x in 0..line.len(){
            if line.as_bytes()[x] == b'['{
                idx = x;
            }
        }

        for dir in line[idx+1..line.len()-1].split(", "){
            if dir != ""{
                ignored_dirs.push(dir.to_string());
            }
        }
        output.push(Drive { fs, drive, mounted_at, ignored_dirs})
    }
    output
}
pub fn save_settings(settings: Settings){
    let file = std::fs::OpenOptions::new().write(true).truncate(true).open(SAVE_SETTINGS_PATH).expect("NO {SAVE_SETTINGS_PATH} found");
    let mut writer = BufWriter::new(file);
    // Write new lines, overwriting everything
    for i in 0..10{
        match i{
            0 => {writeln!(writer, "columns:{:?}",settings.columns).unwrap()}
            1 => {writeln!(writer, "sort_in_use:{:?}",settings.sort_in_use).unwrap()}
            2 => {writeln!(writer, "index_on_startup:{:?}",settings.index_on_startup).unwrap()}
            3 => {writeln!(writer, "index_every_minutes:{:?}",settings.index_every_minutes).unwrap()}
            4 => {writeln!(writer, "instant_search:{:?}",settings.instant_search).unwrap()}
            5 => {writeln!(writer, "journal:{:?}",settings.journal).unwrap()}
            6 => {writeln!(writer, "ignore_case:{:?}",settings.ignore_case).unwrap()}
            7 => {writeln!(writer, "search_full_path:{:?}",settings.search_full_path).unwrap()}
            _ => {}
        }


    }
    writer.flush().unwrap();
}
pub fn load_settings() -> Settings{
    let file = std::fs::File::open(SAVE_SETTINGS_PATH).expect("NO {SAVE_DRIVES_PATH} found");
    let reader = std::io::BufReader::new(file);

    let mut sort_in_use = Sort::default();
    let mut index_on_startup = true;
    let mut index_every_minutes = 0;
    let mut instant_search = true;
    let mut journal = false;
    let mut ignore_case = true;
    let mut columns = Vec::new();
    let mut search_full_path = true;

    let mut i = 0;
    for line in reader.lines(){
        let line = line.unwrap();
        for attr in line.split(':'){
            match i{
                1=>{
                    for c in attr[0..attr.len()-1].split(','){
                        columns.push(c[1..].parse::<u16>().expect(&format!("main.rs:230, {} NaN",c)).clone());
                    }
                }
                3=>{sort_in_use=string_to_sort(attr)}
                5=>{index_on_startup=attr=="true"}
                7=>{index_every_minutes=attr.parse::<u32>().expect("Line {i} is not a number")}
                9=>{instant_search=attr=="true"}
                11=>{journal=attr=="true"}
                13=>{ignore_case=attr=="true"}
                15=>{search_full_path=attr=="true"}
                _ =>{}
            }
            i+= 1;
        }
    }
    Settings{
        columns,
        sort_in_use,
        index_every_minutes,
        index_on_startup,
        instant_search,
        journal,
        ignore_case,
        search_full_path
    }
}
pub fn save_cache(list_of_files: Vec<File>){
    todo!()
}
pub fn load_cache()->Vec<File>{
    todo!()
}