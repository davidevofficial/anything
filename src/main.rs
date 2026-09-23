mod exfat;
mod fat32;
mod ext4;
mod brtfs;
mod ntfs;
mod frontend;
mod backend;
use chrono;

//              maj min patch
const VERSION: (i32,i32,i32) = (3,4,1);

pub fn size_to_pretty_string(size: u64, unit: &UnitSizePreference) -> String{
    match unit{
        UnitSizePreference::KiB1024 => {
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
        UnitSizePreference::KB1000 => {
            if size < 1_000{
                return size.to_string() + "B";
            }
            if size < 1_000_000{
                return format!("{:.2}KB", size as f64 / 1000.0);
            }
            if size < 1_000_000_000{
                return format!("{:.2}MB", size as f64 / 1000000.0);
            }
            if size < 1_000_000_000_000{
                return format!("{:.2}GB", size as f64 / 1000000000.0);
            }else{
                return format!("{:.2}TB", size as f64 / 1000000000000.0);
            }
        }
        UnitSizePreference::KB1024 => {
            if size < 1024{
                return size.to_string() + "B";
            }
            if size < 1048576{
                return format!("{:.2}KB", size as f64 / 1024.0);
            }
            if size < 1073741824{
                return format!("{:.2}MB", size as f64 / 1048576.0);
            }
            if size < 1099511627776{
                return format!("{:.2}GB", size as f64 / 1073741824.0);
            }else{
                return format!("{:.2}TB", size as f64 / 1099511627776.0);
            }
        }
        UnitSizePreference::Kb1000 => {
            let size = size * 8; // Transform to bits
            if size < 1_000{
                return size.to_string() + "b";
            }
            if size < 1_000_000{
                return format!("{:.2}Kb", size as f64 / 1000.0);
            }
            if size < 1_000_000_000{
                return format!("{:.2}Mb", size as f64 / 1000000.0);
            }
            if size < 1_000_000_000_000{
                return format!("{:.2}Gb", size as f64 / 1000000000.0);
            }else{
                return format!("{:.2}Tb", size as f64 / 1000000000000.0);
            }
        }
    }

}
pub fn timestamp_to_string(t: i64)-> String{
    // let secs = (timestamp_ms / 1000); commented it out because apparently the information about ms is not stored inside of the timestamp ???
    // let nanos = ((timestamp_ms % 1000) * 1_000_000) as u32; // ms → ns

    let naive =chrono::DateTime::from_timestamp(t, 0).expect("Invalid Time");
    naive.format("%d/%m/%Y %H:%M:%S").to_string()
}
#[derive(Debug, Default, Clone)]
pub struct Directory{
    name: String
}
#[derive(Debug, Default, Clone)]
pub struct File{
    name: String,
    parent: u32,
    size: u64,
    is_dir: bool,
    create_timestamp: i64,
    last_modified_timestamp: i64,
}
#[derive(Debug, Default, Clone)]
pub struct Drive{
    drive: String,
    mounted_at: String,
    ignored_dirs: Vec<String>
}
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum FilesystemType{
    #[default]
    None,
    Exfat,
    Fat32,
    Ext4,
    Brtfs,
    Ntfs
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
    search_full_path: bool,
    light_mode: bool,
    pixels_per_point: f32,
    dynamic: bool,
    dynamic_factor: u32,
    unit_size_preference: UnitSizePreference,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum UnitSizePreference{
    #[default]
    KiB1024, // IEC standard
    KB1000, // SI standard
    KB1024, // old standard !?
    Kb1000, // Kilobit = 1000bits (speed standard), Mb = 1 million bits
}
impl UnitSizePreference{
    fn string_to_unit_size(string: &str) -> UnitSizePreference{
        match string{
            "KiB1024" => {UnitSizePreference::KiB1024}
            "KB1000" => {UnitSizePreference::KB1000}
            "KB1024" => {UnitSizePreference::KB1024}
            "Kb1000" => {UnitSizePreference::Kb1000}
            _ => {UnitSizePreference::default()}
        }
    }
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


fn main()  {
    println!("CWD: {:?}", env::current_dir().unwrap());
    println!("Binary Location: {:?}", env::current_exe().unwrap());
    let binary_path = env::current_exe().unwrap();
    let parent_dir = binary_path.parent().unwrap();
    let save_settings_path = parent_dir.join("settings").join("settings.txt");
    let save_drives_path = parent_dir.join("settings").join("drives.txt");
    let save_cache_path = parent_dir.join("settings").join("cache.txt");

    // If it is an appimage and settings doesn't exist then write the folder
    if env::var("APPIMAGE").is_ok() && env::var("APPIMAGE").unwrap() != String::new(){
        let appimage_path = env::var("APPIMAGE").unwrap();
        let appimage_path = Path::new(&appimage_path).parent().unwrap();
        let settings_dir = appimage_path.join("settings");
        if !settings_dir.exists(){
            let _ =std::fs::create_dir_all(&settings_dir);
            match std::fs::File::create(&settings_dir.join("settings.txt")){
                Ok(mut file) => {let _ = file.write_all("columns:[200, 950, 100, 150, 150]\nsort_in_use:DateCreatedDescending\nindex_on_startup:true\nindex_every_minutes:60\ninstant_search:true\njournal:false\nignore_case:true\nsearch_full_path:true".as_bytes());}
                Err(e) =>{dbg!(&e);}
            }
            let _ =std::fs::File::create(&settings_dir.join("drives.txt"));
            let _ =std::fs::File::create(&settings_dir.join("cache.txt"));
        }
    // If the folder doesn't exist then create it

    }else{
        if !save_settings_path.exists(){
            let _ =std::fs::create_dir_all(parent_dir.join("settings"));
            match std::fs::File::create(save_settings_path){
                Ok(mut file) => {let _ = file.write_all("columns:[200, 950, 100, 150, 150]\nsort_in_use:DateCreatedDescending\nindex_on_startup:true\nindex_every_minutes:60\ninstant_search:true\njournal:false\nignore_case:true\nsearch_full_path:true".as_bytes());}
                Err(e) =>{dbg!(&e);}
            }
            let _ =std::fs::File::create(save_drives_path);
            let _ =std::fs::File::create(save_cache_path);
        }
    };

    let _ = frontend::start_frontend();
}

/// Returns the path of the appimage/binary (and wether it is an appimage)
fn am_i_an_appimage()->(bool, String){
    // If it is an appimage it will have the APPIMAGE env var apparently
    if env::var("APPIMAGE").is_ok() && env::var("APPIMAGE").unwrap() != String::new(){
        let appimage_path = env::var("APPIMAGE").unwrap();
        let parent_dir = Path::new(&appimage_path).parent().unwrap();
        return (true, parent_dir.to_str().unwrap().to_string());
    }
    let binary_path = env::current_exe().unwrap();
    let parent_dir = binary_path.parent().unwrap();
    return (false, parent_dir.to_str().unwrap().to_string());
}

use std::io::{BufRead, BufWriter, Write};
use std::env;
use std::path::Path;
pub fn save_drives(drives: Vec<Drive>){
    let save_drives_path = Path::new(&am_i_an_appimage().1).join("settings").join("drives.txt");
    let file = match std::fs::OpenOptions::new().write(true).truncate(true).open(save_drives_path){
        Ok(a) => {a},
        Err(e) =>{
            println!("Error while saving drives");
            dbg!(e);
            return;
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
        writeln!(writer, "{}\t{}\t{}",
            drive.drive, drive.mounted_at, s).unwrap();

    }
    writer.flush().unwrap();
}
pub fn load_drives() -> Vec<Drive>{
    let mut output = Vec::new();
    let save_drives_path = Path::new(&am_i_an_appimage().1).join("settings").join("drives.txt");
    let file = match std::fs::File::open(save_drives_path){
        Ok(a) => {a},
        Err(e) =>{
            println!("Error while loading drives");
            dbg!(e);
            return Vec::new();
        }
    };

    let reader = std::io::BufReader::new(file);
    for line in reader.lines(){
        let line = line.unwrap();
        let mut ignored_dirs = Vec::new();

        let attr: Vec<&str> = line.splitn(3, '\t').collect();
        let drive = attr[0].to_string();
        let mounted_at = attr[1].to_string();
        for dir in attr[2][1..attr[2].len()-1].split(", "){
            if dir != ""{
                ignored_dirs.push(dir.to_string());
            }
        }

        output.push(Drive { drive, mounted_at, ignored_dirs})
    }
    output
}
pub fn save_settings(settings: Settings){
    let save_settings_path = Path::new(&am_i_an_appimage().1).join("settings").join("settings.txt");
    let file = match std::fs::OpenOptions::new().write(true).truncate(true).open(save_settings_path){
        Ok(a) => {a},
        Err(e) =>{
            println!("Error while saving settings");
            dbg!(e);
            return;
        }
    };

    let mut writer = BufWriter::new(file);
    writeln!(writer, "columns:{:?}",settings.columns).unwrap();
    writeln!(writer, "sort_in_use:{:?}",settings.sort_in_use).unwrap();
    writeln!(writer, "index_on_startup:{:?}",settings.index_on_startup).unwrap();
    writeln!(writer, "index_every_minutes:{:?}",settings.index_every_minutes).unwrap();
    writeln!(writer, "instant_search:{:?}",settings.instant_search).unwrap();
    writeln!(writer, "journal:{:?}",settings.journal).unwrap();
    writeln!(writer, "ignore_case:{:?}",settings.ignore_case).unwrap();
    writeln!(writer, "search_full_path:{:?}",settings.search_full_path).unwrap();
    writeln!(writer, "light_mode:{:?}",settings.light_mode).unwrap();
    writeln!(writer, "pixels_per_point:{:?}",settings.pixels_per_point).unwrap();
    writeln!(writer, "dynamic:{:?}",settings.dynamic).unwrap();
    writeln!(writer, "dynamic_factor:{:?}",settings.dynamic_factor).unwrap();
    writeln!(writer, "unit_size_preference:{:?}",settings.unit_size_preference).unwrap();

    writer.flush().unwrap();
}
pub fn load_settings() -> Settings{

    let mut sort_in_use = Sort::default();
    let mut index_on_startup = true;
    let mut index_every_minutes = 0;
    let mut instant_search = true;
    let mut journal = false;
    let mut ignore_case = true;
    let mut columns = Vec::new();
    let mut search_full_path = true;
    let mut light_mode = true;
    let mut pixels_per_point = 1.0;
    let mut dynamic = false;
    let mut dynamic_factor = 100;
    let mut unit_size_preference = UnitSizePreference::default();

    let save_settings_path = Path::new(&am_i_an_appimage().1).join("settings").join("settings.txt");
    let file = match std::fs::File::open(save_settings_path){
        Ok(a) => {a},
        Err(e) =>{
            println!("Error while loading settings");
            dbg!(e);
            return Settings{
                columns,
                sort_in_use,
                index_every_minutes,
                index_on_startup,
                instant_search,
                journal,
                ignore_case,
                search_full_path,
                light_mode,
                pixels_per_point,
                dynamic,
                dynamic_factor,
                unit_size_preference
            }
        }
    };

    let reader = std::io::BufReader::new(file);

    for line in reader.lines(){
        let line = line.unwrap();
        let attr: Vec<&str> = line.split(':').collect();
        match attr[0] {
            "columns" => {
                for c in attr[1].split(','){
                    if !c.ends_with(']'){
                        columns.push(c[1..].parse::<u16>().expect(&format!("main.rs:380, {} NaN",c)).clone());
                    }else{
                        columns.push(c[1..c.len()-1].parse::<u16>().expect(&format!("main.rs:382, {} NaN",c)).clone());
                    }
                }
            }
            "sort_in_use" => {sort_in_use=string_to_sort(attr[1])}
            "unit_size_preference" => {unit_size_preference = UnitSizePreference::string_to_unit_size(attr[1])}
            "index_on_startup" => {index_on_startup=attr[1]=="true"}
            "index_every_minutes"=>{index_every_minutes=attr[1].parse::<u32>().expect("Line {i} is not a number")}
            "instant_search"=>{instant_search=attr[1]=="true"}
            "journal"=>{journal=attr[1]=="true"}
            "ignore_case"=>{ignore_case=attr[1]=="true"}
            "search_full_path"=>{search_full_path=attr[1]=="true"}
            "light_mode"=>{light_mode=attr[1]=="true"}
            "pixels_per_point"=>{pixels_per_point=attr[1].parse::<f32>().expect("Line {i} is not a float")}
            "dynamic" => {dynamic=attr[1]=="true"}
            "dynamic_factor" =>{dynamic_factor=attr[1].parse::<u32>().expect("Line {i} is not a number")}
            _ =>{}
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
        search_full_path,
        light_mode,
        pixels_per_point,
        dynamic,
        dynamic_factor,
        unit_size_preference
    }
}
pub fn save_cache(list_of_files: Vec<File>, list_of_directories: Vec<Directory>){
    let save_settings_path = Path::new(&am_i_an_appimage().1).join("settings").join("cache.txt");
    let file = match std::fs::OpenOptions::new().write(true).truncate(true).open(save_settings_path){
        Ok(a) => {a},
        Err(e) =>{
            println!("Error while saving cache");
            dbg!(e);
            return;
        }
    };
    let mut writer = BufWriter::new(file);
    let _ = writer.write_all(&(list_of_directories.len() as u32).to_le_bytes());
    let _ = writeln!(&mut writer, "");

    for d in list_of_directories{
        let _ = writeln!(&mut writer, "{}",d.name);
    }
    for f in list_of_files{
        let size = f.size;
        let t_created = f.create_timestamp;
        let t_modified = f.last_modified_timestamp;
        let name = f.name;
        let size_bytes = size.to_le_bytes();
        let t_created_bytes = t_created.to_le_bytes();
        let t_modified_bytes = t_modified.to_le_bytes();
        let parent_idx = f.parent.to_le_bytes();
        // let s = format!("{size_bytes}{t_created_bytes}{t_modified_bytes}{name}");
        let _ = writer.write_all(&size_bytes);
        let _ = writer.write_all(&t_created_bytes);
        let _ = writer.write_all(&t_modified_bytes);
        let _ = writer.write_all(&parent_idx);
        let _ = writeln!(&mut writer, "{}",name);
    }
}
pub fn load_cache()->(Vec<File>, Vec<Directory>){
    let save_cache_path = Path::new(&am_i_an_appimage().1).join("settings").join("cache.txt");
    let _file = match std::fs::OpenOptions::new().read(true).open(&save_cache_path){
        Ok(a) => {a},
        Err(e) =>{
            println!("Error while loading cache");
            dbg!(e);
            return (Vec::new(), Vec::new());
        }
    };
    let file = std::fs::read(save_cache_path).unwrap();
    if file.len() == 0{
        return (vec![], vec![]);
    }
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let directories_n = u32::from_le_bytes([file[0],file[1],file[2],file[3]]);
    let mut i = 5;
    loop {
        let mut name_bytes = Vec::new();
        while i < file.len() && file[i] != b'\n' {
            name_bytes.push(file[i]);
            i += 1;
        }
        i += 1; //Skip the null terminator
        let name = String::from_utf8_lossy(&name_bytes).to_string();
        directories.push(Directory { name });
        if directories.len() as u32 == directories_n{break;}
    }
    let mut p = i;
    while p + 28 < file.len(){
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
        let parent = u32::from_le_bytes([file[p+24],file[p+25],file[p+26],file[p+27]]);
        p += 28;
        // Read null-terminated UTF-8
        let mut name_bytes = Vec::new();
        while p < file.len() && file[p] != b'\n' {
            name_bytes.push(file[p]);
            p += 1;
        }
        p += 1;  // Skip null terminator
        let name = String::from_utf8_lossy(&name_bytes).to_string();
        let mut is_dir = false;
        if name.ends_with("/"){
            is_dir = true;
        }
        files.push(File{
            name,
            parent,
            size,is_dir,
            create_timestamp:t_created,
            last_modified_timestamp: t_modified
        })
    }
    (files, directories)
}