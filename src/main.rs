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
pub fn timestamp_to_string(t: i64)-> String{
    // let secs = (timestamp_ms / 1000); commented it out because apparently the information about ms is not stored inside of the timestamp ???
    // let nanos = ((timestamp_ms % 1000) * 1_000_000) as u32; // ms → ns

    let naive =chrono::DateTime::from_timestamp(t, 0).expect("Invalid Time");
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
    // If it is an appimage and settings doesn't exist then write the folder
    if env::var("APPIMAGE").is_ok() && env::var("APPIMAGE").unwrap() != String::new(){
        let appimage_path = env::var("APPIMAGE").unwrap();
        let appimage_path = Path::new(&appimage_path);
        let settings_dir = appimage_path.join("settings");
        if !settings_dir.exists(){
            let _ =std::fs::create_dir_all(&settings_dir);
            let mut file = std::fs::File::create(&settings_dir.join("settings.txt")).unwrap();
            let _ = file.write_all("columns:[200, 950, 100, 150, 150]\nsort_in_use:SizeAscending\nindex_on_startup:true\nindex_every_minutes:60\ninstant_search:true\njournal:false\nignore_case:true\nsearch_full_path:true".as_bytes());
            let _ =std::fs::File::create(&settings_dir.join("drives.txt"));
            let _ =std::fs::File::create(&settings_dir.join("cache.txt"));
        }
    }else{
        if !Path::new(SAVE_SETTINGS_PATH).exists(){
            let _ =std::fs::create_dir_all("./settings");
            let mut file = std::fs::File::create(SAVE_SETTINGS_PATH).unwrap();
            let _ = file.write_all("columns:[200, 950, 100, 150, 150]\nsort_in_use:SizeAscending\nindex_on_startup:true\nindex_every_minutes:60\ninstant_search:true\njournal:false\nignore_case:true\nsearch_full_path:true".as_bytes());
            let _ =std::fs::File::create(SAVE_DRIVES_PATH);
            let _ =std::fs::File::create(SAVE_CACHE_PATH);
        }
    };

    let _ = frontend::start_frontend();
}

use std::io::{BufRead, BufWriter, Write};
use std::env;
use std::path::Path;
pub fn save_drives(drives: Vec<Drive>){
    let file = match std::fs::OpenOptions::new().write(true).truncate(true).open(SAVE_DRIVES_PATH){
        Ok(a) => {a},
        Err(_) =>{
            // If it is an appimage
            let appimage_path = env::var("APPIMAGE")
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "APPIMAGE env var not set")).unwrap();
            let appimage_path = Path::new(&appimage_path);
            let app_dir = appimage_path.parent().unwrap();
            let settings_dir = app_dir.join("settings");
            let path = settings_dir.join("drives.txt");
            dbg!(&path);
            std::fs::OpenOptions::new().write(true).truncate(true).open(path).unwrap()
        }
    };

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

    let file = match std::fs::File::open(SAVE_DRIVES_PATH){
        Ok(a) => {a},
        Err(_) =>{
            // If it is an appimage
            let appimage_path = env::var("APPIMAGE")
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "APPIMAGE env var not set")).unwrap();
            let appimage_path = Path::new(&appimage_path);
            let app_dir = appimage_path.parent().unwrap();
            let settings_dir = app_dir.join("settings");
            let path = settings_dir.join("drives.txt");
            std::fs::File::open(path).unwrap()
        }
    };

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

    let file = match std::fs::OpenOptions::new().write(true).truncate(true).open(SAVE_SETTINGS_PATH){
        Ok(a) => {a},
        Err(_) =>{
            // If it is an appimage
            let appimage_path = env::var("APPIMAGE")
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "APPIMAGE env var not set")).unwrap();
            let appimage_path = Path::new(&appimage_path);
            let app_dir = appimage_path.parent().unwrap();
            let settings_dir = app_dir.join("settings");
            let path = settings_dir.join("settings.txt");
            std::fs::OpenOptions::new().write(true).truncate(true).open(path).unwrap()
        }
    };

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

    let file = match std::fs::File::open(SAVE_SETTINGS_PATH){
        Ok(a) => {a},
        Err(_) =>{
            // If it is an appimage
            let appimage_path = env::var("APPIMAGE")
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "APPIMAGE env var not set")).unwrap();
            let appimage_path = Path::new(&appimage_path);
            let app_dir = appimage_path.parent().unwrap();
            let settings_dir = app_dir.join("settings");
            let path = settings_dir.join("settings.txt");
            std::fs::File::open(path).unwrap()
        }
    };

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
    let file = match std::fs::OpenOptions::new().write(true).truncate(true).open(SAVE_CACHE_PATH){
        Ok(a) => {a},
        Err(_) =>{
            // If it is an appimage
            let appimage_path = env::var("APPIMAGE")
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "APPIMAGE env var not set")).unwrap();
            let appimage_path = Path::new(&appimage_path);
            let app_dir = appimage_path.parent().unwrap();
            let settings_dir = app_dir.join("settings");
            let path = settings_dir.join("cache.txt");
            std::fs::OpenOptions::new().write(true).truncate(true).open(path).unwrap()
        }
    };
    let mut writer = BufWriter::new(file);
    for f in list_of_files{
        let size = f.size;
        let t_created = f.create_timestamp;
        let t_modified = f.last_modified_timestamp;
        let name = f.full_name;
        let size_bytes = size.to_le_bytes();
        let t_created_bytes = t_created.to_le_bytes();
        let t_modified_bytes = t_modified.to_le_bytes();
        // let s = format!("{size_bytes}{t_created_bytes}{t_modified_bytes}{name}");
        let _ = writer.write_all(&size_bytes);
        let _ = writer.write_all(&t_created_bytes);
        let _ = writer.write_all(&t_modified_bytes);
        let _ = writeln!(&mut writer, "{}",name);
    }
}
pub fn load_cache()->Vec<File>{

    let file = match std::fs::read(SAVE_CACHE_PATH){
        Ok(a) => {a},
        Err(_) =>{
            // If it is an appimage
            let appimage_path = env::var("APPIMAGE")
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "APPIMAGE env var not set")).unwrap();
            let appimage_path = Path::new(&appimage_path);
            let app_dir = appimage_path.parent().unwrap();
            let settings_dir = app_dir.join("settings");
            let path = settings_dir.join("cache.txt");
            std::fs::read(path).unwrap()
        }
    };
    let mut p = 0;
    let mut files = Vec::new();
    while p + 24 < file.len(){
        let size =  u64::from_le_bytes([
                file[p+0],file[p+1],file[p+2],file[p+3],
                file[p+4],file[p+5],file[p+6],file[p+7]
        ]);
        let t_created =  i64::from_le_bytes([
                file[p+8],file[p+9],file[p+10],file[p+11],
                file[p+12],file[p+13],file[p+14],file[p+15]
        ]);
        let t_modified =  i64::from_le_bytes([
                file[p+16],file[p+17],file[p+18],file[p+19],
                file[p+20],file[p+21],file[p+22],file[p+23]
        ]);
        p += 24;
        // Read null-terminated UTF-8
        let mut name_bytes = Vec::new();
        while p < file.len() && file[p] != b'\n' {
            name_bytes.push(file[p]);
            p += 1;
        }
        p += 1;  // Skip null terminator
        let full_name = String::from_utf8_lossy(&name_bytes).to_string();
        let mut is_dir = false;
        let mut name = String::new();
        if full_name.ends_with("/") {
            is_dir = true;
            let parts: Vec<&str> = full_name.rsplitn(3, '/').collect();
            name = parts[1].to_string();
        }else{
            let parts: Vec<&str> = full_name.rsplitn(2,'/').collect();
            name = parts[0].to_string();
        }
        files.push(File{
            name,
            full_name,
            size,is_dir,
            create_timestamp:t_created,
            last_modified_timestamp: t_modified
        })
    }
    files
}