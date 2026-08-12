use std::thread;
use crate::{self as main, FilesystemType, exfat, ntfs, ext4, fat32, brtfs};

/// If it can read on github the version.txt then it returns Some( maj, min, patch )
pub fn check_current_verson() -> Option<(i32,i32,i32)>{
    let url = "https://raw.githubusercontent.com/davidevofficial/anything/master/version.txt";
    let output = std::process::Command::new("curl")
        .arg("-sL") // silent, follow redirects
        .arg(url)
        .output();
    if output.is_err(){return None;}
    let c = String::from_utf8(output.unwrap().stdout);
    if c.is_err(){return None;}
    let c = c.unwrap();
    for line in c.lines(){
        let version: Vec<&str> = line.splitn(3, ".").collect();
        let maj = version[0].parse::<i32>();
        if maj.is_err(){return None;}
        let maj = maj.unwrap();
        let min = version[1].parse::<i32>();
        if min.is_err(){return None;}
        let min = min.unwrap();
        let patch = version[2].parse::<i32>();
        if patch.is_err(){return None;}
        let patch = patch.unwrap();
        return Some((maj, min, patch));
    }
    return None;
}

fn check_drive_filesystem_type(drive: String) -> FilesystemType{
    if exfat::is_drive_valid(drive.clone()){
        return FilesystemType::Exfat;
    }
    else if ntfs::is_drive_valid(drive.clone()){
        return FilesystemType::Ntfs;
    }
    else if ext4::is_drive_valid(drive.clone()){
        return FilesystemType::Ext4;
    }
    else if fat32::is_drive_valid(drive.clone()){
        return FilesystemType::Fat32;
    }
    else if brtfs::is_drive_valid(drive.clone()){
        return FilesystemType::Brtfs;
    }
    else {
        return FilesystemType::None;
    }
}

/// String must be of type yyyy-mm-dd hour:minute:second
/// Converts from user-legible string to epoch for file searching
fn date_to_epoch(s: &str) -> i64{
    use chrono::NaiveDateTime;
    use chrono::NaiveDate;
    //  dbg!(dt);
    for i in 0..15{
        match i {
            0 => {
                let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S");
                if dt.is_ok(){
                    return dt.unwrap().and_utc().timestamp()
                    }
                }
            1 => {
                let dt = NaiveDateTime::parse_from_str(s, "%Y/%m/%d %H:%M:%S");                if dt.is_ok(){
                    return dt.unwrap().and_utc().timestamp()
                    }
                }
            2 => {
                let dt = NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S");
                if dt.is_ok(){
                    return dt.unwrap().and_utc().timestamp()
                    }
                }
            3 => {
                let dt = NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H-%M-%S");
                if dt.is_ok(){
                    return dt.unwrap().and_utc().timestamp()
                    }
                }
            4 => {
                let dt = NaiveDate::parse_from_str(s, "%Y/%m/%d");
                if dt.is_ok(){
                    return dt.unwrap().to_epoch_days() as i64 * 86400
                }
            }
            5 => {
                let dt = NaiveDate::parse_from_str(s, "%Y:%m:%d");if dt.is_ok(){
                return dt.unwrap().to_epoch_days() as i64 * 86400
                }
            }
            6 => {
                let dt = NaiveDate::parse_from_str(s, "%Y-%m-%d");
                if dt.is_ok(){
                    return dt.unwrap().to_epoch_days() as i64 * 86400
                }
            }
            7 => {
                let dt = NaiveDateTime::parse_from_str(s, "%Y-%d-%m %H:%M:%S");
                if dt.is_ok(){
                    return dt.unwrap().and_utc().timestamp()
                    }
                }
            8 => {
                let dt = NaiveDateTime::parse_from_str(s, "%Y/%d/%m %H:%M:%S");
                if dt.is_ok(){
                    return dt.unwrap().and_utc().timestamp()
                    }
                }
            9 => {
                let dt = NaiveDateTime::parse_from_str(s, "%Y:%d:%m %H:%M:%S");
                if dt.is_ok(){
                    return dt.unwrap().and_utc().timestamp()
                    }
                }
            10 => {
                let dt = NaiveDate::parse_from_str(s, "%Y/%d/%m");
                if dt.is_ok(){
                    return dt.unwrap().to_epoch_days() as i64 * 86400
                }
            }
            11 => {
                let dt = NaiveDate::parse_from_str(s, "%Y:%d:%m");
                if dt.is_ok(){
                    return dt.unwrap().to_epoch_days() as i64 * 86400
                }
            }
            12 => {
                let dt = NaiveDate::parse_from_str(s, "%Y-%d-%m");
                if dt.is_ok(){
                    return dt.unwrap().to_epoch_days() as i64 * 86400
                }
            }
            _ => {return 0;}
        }
    }
    return 0;
}

fn wrapper_parse_u64(s: &str) -> u64{
    let n = s.parse::<u64>();
    if n.is_ok(){
        return n.unwrap()
    }else{0}
}

pub const KB: u64 = 1_000;
pub const MB: u64 = 1_000_000;
pub const GB: u64 = 1_000_000_000;
pub const TB: u64 = 1_000_000_000_000;
/// From human readable to bytes (u64) lowercase or uppercase doesn't matter
/// 123b = 123, 123 = 123
/// 123k = 123.000, 123Kb = 123.000
/// 7m = 7.000.000, 7Mb = 7.000.000
/// 7g = 7.000.000.000, 7Gb = 7.000.000.000
/// 1t = 1.000.000.000.000, 1Tb = 1.000.000.000.000
fn string_to_size_in_bytes(s: &str) -> u64{
    let s = s.to_lowercase();

    // Terabyte = 1_000_000_000_000 Bytes
    if s.ends_with('t'){
        return wrapper_parse_u64(&s[0..s.len()-1]) * TB;
    }else if s.ends_with("tb"){
        return wrapper_parse_u64(&s[0..s.len()-2]) * TB;
    }
    // Gigabyte = 1_000_000_000 Bytes
    if s.ends_with('g'){
        return wrapper_parse_u64(&s[0..s.len()-1]) * GB;
    }else if s.ends_with("gb"){
        return wrapper_parse_u64(&s[0..s.len()-2]) * GB;
    }
    // Megabyte = 1_000_000 Bytes
    if s.ends_with('m'){
        return wrapper_parse_u64(&s[0..s.len()-1]) * MB;
    }else if s.ends_with("mb"){
        return wrapper_parse_u64(&s[0..s.len()-2]) * MB;
    }
    // Kilobytes = 1000 Bytes
    if s.ends_with('k'){
        return wrapper_parse_u64(&s[0..s.len()-1]) * KB;
    }else if s.ends_with("kb"){
        return wrapper_parse_u64(&s[0..s.len()-2]) * KB;
    }
    // Bytes = 1 Byte
    if s.ends_with('b'){
        return wrapper_parse_u64(&s[0..s.len()-1]);
    }
    return wrapper_parse_u64(&s[0..s.len()]);

}

#[derive(Debug, Clone, Default)]
enum FilterType{
    BiggerThan(u64),
    SmallerThan(u64),
    IsFolder,
    IsFile,
    CreatedBefore(i64),
    CreatedAfter(i64),
    ModifiedAfter(i64),
    ModifiedBefore(i64),
    StartsWith(String),
    EndsWith(String),
    #[default]
    None
}

impl FilterType{
    fn from_string(s: &str) -> FilterType{
        let s = s.to_lowercase();
        let s = s.trim();
        // Starts or Ends with (*)
        if s.contains('*'){
            // Divides in two strings, if the first is empty then it means that it is *_ otherwise _*
            let v: Vec<&str> = s.splitn(2, '*').collect();
            if v[0].is_empty(){
                return FilterType::EndsWith(v[1].to_string());
            }else{
                return FilterType::StartsWith(v[0].to_string())
            }
        }
        // folder/file
        if s.contains("folder"){return FilterType::IsFolder}
        if s.contains("file"){return FilterType::IsFile}
        // other filters
        // bigger than
        if s.contains('>'){
            let mut parts: Vec<&str> = s.splitn(2, '>').collect();
            parts[0] = parts[0].trim();
            parts[1] = parts[1].trim();
            match parts[0]{
                "s" | "size" => {
                    let size = string_to_size_in_bytes(parts[1]);
                    return FilterType::BiggerThan(size);
                }
                "c" | "creation" => {
                    let date = date_to_epoch(parts[1]);
                    return FilterType::CreatedAfter(date);
                }
                "m" | "modified" => {
                    let date = date_to_epoch(parts[1]);
                    return FilterType::ModifiedAfter(date);
                }
                _ => {}
            }
        }
        // smaller than
        if s.contains('<'){
            let mut parts: Vec<&str> = s.splitn(2, '<').collect();
            parts[0] = parts[0].trim();
            parts[1] = parts[1].trim();
            match parts[0]{
                "s" | "size" => {
                    let size = string_to_size_in_bytes(parts[1]);
                    return FilterType::SmallerThan(size);
                }
                "c" | "creation" => {
                    let date = date_to_epoch(parts[1]);
                    return FilterType::CreatedBefore(date);
                }
                "m" | "modified" => {
                    let date = date_to_epoch(parts[1]);
                    return FilterType::ModifiedBefore(date);
                }
                _ => {}
            }
        }

        FilterType::None
    }
}

#[derive(Debug, Clone)]
struct SearchFilter{
    search_string: String,
    negation: bool,
    filter: FilterType
}
impl SearchFilter{
    fn default(search_string: String) -> SearchFilter{
        return SearchFilter { search_string, negation: false, filter: FilterType::default() }
    }
}

fn string_to_predicates(s: String) -> Vec<SearchFilter>{
    let mut output = Vec::new();
    if !s.contains('\\'){
        let f = SearchFilter::default(s);
        output.push(f);
    }else{
        let predicates: Vec<&str> = s.split('\\').collect();
        for p in predicates{
            if p.is_empty(){
                continue;
            }
            let mut negation = false;
            if p.starts_with('!'){
                negation = true;
            }
            if p.contains('(') && p.contains(')'){
                let mut left_parent_idx = 1;
                if negation{
                    left_parent_idx = 2;
                }
                let parts: Vec<&str> = p[left_parent_idx..].splitn(2,')').collect();
                let filter = FilterType::from_string(parts[0]);
                let search_string = parts[1].to_string();
                output.push(SearchFilter { search_string, negation, filter })
            }else{
                if negation{
                    let s = &p[1..];
                    output.push( SearchFilter { search_string: s.to_string(), negation, filter: FilterType::None })
                }else{
                    let s = p;
                    output.push( SearchFilter { search_string: s.to_string(), negation, filter: FilterType::None })
                }
            }
        }
    }
    output
}
fn filter_match(item: &main::File, predicate: SearchFilter) -> bool{
    if !predicate.negation{
        match predicate.filter{
            FilterType::IsFolder => {return item.is_dir && item.name.contains(predicate.search_string.as_str())}
            FilterType::IsFile => {return !item.is_dir && item.name.contains(predicate.search_string.as_str())}
            FilterType::BiggerThan(x) => {return item.size > x && item.name.contains(predicate.search_string.as_str())}
            FilterType::SmallerThan(x) => {return item.size < x && item.name.contains(predicate.search_string.as_str())}
            FilterType::EndsWith(y) => {return item.name.ends_with(y.as_str()) && item.name.contains(predicate.search_string.as_str())}
            FilterType::StartsWith(y) => {return item.name.starts_with(y.as_str()) && item.name.contains(predicate.search_string.as_str())}
            FilterType::ModifiedAfter(x) => {return item.last_modified_timestamp > x && item.name.contains(predicate.search_string.as_str())}
            FilterType::ModifiedBefore(x) => {return item.last_modified_timestamp < x && item.name.contains(predicate.search_string.as_str())}
            FilterType::CreatedAfter(x) => {return item.create_timestamp > x && item.name.contains(predicate.search_string.as_str())}
            FilterType::CreatedBefore(x) => {return item.create_timestamp < x && item.name.contains(predicate.search_string.as_str())}
            FilterType::None => {return item.name.contains(predicate.search_string.as_str())}
        }
    }else{
        match predicate.filter{
            FilterType::IsFolder => {return !item.is_dir && item.name.contains(predicate.search_string.as_str())}
            FilterType::IsFile => {return item.is_dir && item.name.contains(predicate.search_string.as_str())}
            FilterType::BiggerThan(x) => {return item.size < x && item.name.contains(predicate.search_string.as_str())}
            FilterType::SmallerThan(x) => {return item.size > x && item.name.contains(predicate.search_string.as_str())}
            FilterType::EndsWith(y) => {return !item.name.ends_with(y.as_str()) && item.name.contains(predicate.search_string.as_str())}
            FilterType::StartsWith(y) => {return !item.name.starts_with(y.as_str()) && item.name.contains(predicate.search_string.as_str())}
            FilterType::ModifiedAfter(x) => {return item.last_modified_timestamp < x && item.name.contains(predicate.search_string.as_str())}
            FilterType::ModifiedBefore(x) => {return item.last_modified_timestamp > x && item.name.contains(predicate.search_string.as_str())}
            FilterType::CreatedAfter(x) => {return item.create_timestamp < x && item.name.contains(predicate.search_string.as_str())}
            FilterType::CreatedBefore(x) => {return item.create_timestamp > x && item.name.contains(predicate.search_string.as_str())}
            FilterType::None => {return !item.name.contains(predicate.search_string.as_str())}
        }
    }
}
pub fn search(items: Vec<main::File>, directories: Vec<main::Directory>, settings: main::Settings, searching_for: String,cancel_flag: std::sync::mpsc::Receiver<u8>)->Vec<usize>{
    let mut output: Vec<usize> = Vec::new();
    let predicates = string_to_predicates(searching_for.clone());

    dbg!(predicates.clone());
    if predicates.len() == 1{
        for j in 0..items.len(){
            match cancel_flag.try_recv(){
                Ok(1) => {return output;}
                _=>{}
            }
            let mut search_filter = predicates[0].clone();
            let mut item = items[j].clone();
            if settings.ignore_case{
                search_filter.search_string = search_filter.search_string.to_lowercase();
                if settings.search_full_path{
                    item.name = directories[item.parent as usize].name.clone() + &item.name;
                }
                item.name = item.name.to_lowercase();
            }
            if filter_match(&item, search_filter.clone()){
                output.push(j);
            }
        }
    } else if predicates.len() > 1{
        // todo!();
        for i in 0..predicates.len(){
            if i == 0{
                //start caching
                for j in 0..items.len(){
                    match cancel_flag.try_recv(){
                        Ok(1) => {return output;}
                        _=>{}
                    }
                    let mut search_filter = predicates[0].clone();
                    let mut item = items[j].clone();
                    if settings.ignore_case{
                        search_filter.search_string = search_filter.search_string.to_lowercase();
                        if settings.search_full_path{
                            item.name = directories[item.parent as usize].name.clone() + &item.name;
                        }
                        item.name = item.name.to_lowercase();
                    }
                    if filter_match(&item, search_filter.clone()){
                        output.push(j);
                    }
                }
            } else{
                //use cache
                let mut temp: Vec<usize> = Vec::new();
                for o in &output{
                    match cancel_flag.try_recv(){
                        Ok(1) => {return temp;}
                        _=>{}
                    }
                    let mut search_filter = predicates[i].clone();
                    let mut item: main::File = items[*o].clone();
                    if settings.ignore_case{
                        search_filter.search_string = search_filter.search_string.to_lowercase();
                        if settings.search_full_path{
                            item.name = directories[item.parent as usize].name.clone() + &item.name;
                        }
                        item.name = item.name.to_lowercase();
                    }
                    if filter_match(&item, search_filter.clone()){
                        temp.push(*o);
                    }
                }
                output = temp;
            }
        }
    }
    output
}

pub fn index_drives(drives: Vec<main::Drive>)->(Vec<main::File>, Vec<main::Directory>, u32){
    let mut items: (Vec<main::File>, Vec<main::Directory>, u32) = (Vec::new(), Vec::new(), 0);
    let mut vec = Vec::new();
    for d in drives{
        vec.push((d.drive.clone(), d.mounted_at.clone(), d.ignored_dirs.clone()));
    }
    let handles: Vec<_> = vec
        .into_iter()
        .map(|(drive, mounted_at, ignored_dirs)| {
            thread::spawn(move || index(drive, mounted_at, ignored_dirs))
        })
        .collect();

    let temp_items: Vec<(Vec<main::File>, Vec<main::Directory>, u32)> = handles
        .into_iter()
        .map(|h| h.join().expect("thread panicked"))
        .collect();

    let mut idx = 0;
    let mut err = 0;
    for t in &temp_items{
        err += t.2;
        for f in &t.0{
            let mut f = f.clone();
            f.parent += idx;
            items.0.push(f);
        }
        idx += t.1.len() as u32;
    }
    for mut t in temp_items{
        items.1.append(&mut t.1);
    }
    items.2 = err;
    items
}

fn index(mut drive: String, mounted_at: String, ignored_dirs: Vec<String>)
    -> (Vec<main::File>, Vec<main::Directory>, u32){
    let mut items = (Vec::new(), Vec::new(), 0);
    let mut found = false;
    let temp_drives = get_devices();
    for t in temp_drives.clone(){
        if t.mounted_at == mounted_at{
            found = true;
            if drive != t.drive{
                drive = t.drive;
            }
        }
    }
    if !found{
        items.2+=1;
        return items;
    }
    let fs = check_drive_filesystem_type(drive.clone());
    match fs{
        FilesystemType::None =>{items.2 += 1;}
        FilesystemType::Exfat => {
            let result = main::exfat::index(drive.clone(), mounted_at.clone(), ignored_dirs);
            if result.is_err(){
                items.2 += result.err().unwrap();
            }else{
                let (mut files, mut dir) = result.unwrap();
                items.0.append(&mut files);
                items.1.append(&mut dir);
            }
        }
        FilesystemType::Ext4 => {
            let result = main::ext4::index(drive.clone(), mounted_at.clone(), ignored_dirs);
            if result.is_err(){
                items.2 += result.err().unwrap();
            }else{
                let (mut files, mut dir) = result.unwrap();
                items.0.append(&mut files);
                items.1.append(&mut dir);
            }
        }
        FilesystemType::Ntfs => {
            let result = main::ntfs::index(drive.clone(), mounted_at.clone(), ignored_dirs);
            if result.is_err(){
                items.2 += result.err().unwrap();
            }else{
                let (mut files, mut dir) = result.unwrap();
                items.0.append(&mut files);
                items.1.append(&mut dir);
            }
        }
        FilesystemType::Fat32 => {
            let result = main::fat32::index(drive.clone(), mounted_at.clone(), ignored_dirs);
            if result.is_err(){
                items.2 += result.err().unwrap();
            }else{
                let (mut files, mut dir) = result.unwrap();
                items.0.append(&mut files);
                items.1.append(&mut dir);
            }
        }
        FilesystemType::Brtfs => {
            let result = main::brtfs::index(drive.clone(), mounted_at.clone(), ignored_dirs);
            if result.is_err(){
                items.2 += result.err().unwrap();
            }else{
                let (mut files, mut dir) = result.unwrap();
                items.0.append(&mut files);
                items.1.append(&mut dir);
            }
        }
    }
    println!("Drive: {}, Using {:?} Filesystem, Mounted at: {} finished indexing!", drive, fs, mounted_at);
    items
}
pub fn get_devices()->Vec<main::Drive>{
    let lsblk = std::process::Command::new("lsblk")
        .args(&["-l", "-n", "-o", "PATH,MOUNTPOINT"])
        .output()
        .expect("lsblk failed");
    let mut drives = Vec::new();
    let lines = lines_from_bytes(lsblk.stdout);
    for i in 0..lines.len(){
        if lines[i].contains(&b'/'){
            if lines[i][5] == b's' && lines[i][6] == b'd' || lines[i][5] == b'n' || lines[i][5] == b'm'{
                let mut space = 0;
                for j in 0..lines[i].len(){
                    if lines[i][j] == b' '{space=j;break;}
                }
                let drive = &lines[i][0..space];
                let mut slash = 0;
                for j in space..lines[i].len(){
                    if lines[i][j] == b'/'{slash=j;break;}
                }
                if slash == 0{continue}
                let mounted_at = &lines[i][slash..lines[i].len()-1];
                let drive = drive.to_vec();
                let drive = String::from_utf8(drive).unwrap();
                let mounted_at = String::from_utf8(mounted_at.to_vec()).unwrap();
                drives.push(main::Drive{drive,mounted_at,ignored_dirs:vec![]});
            }
        }
    }
    drives
}

fn lines_from_bytes(mut data: Vec<u8>) -> Vec<Vec<u8>> {
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