use crate::{self as main, FilesystemType, exfat, ntfs, ext4};

fn check_drive_filesystem_type(drive: String) -> FilesystemType{
    if exfat::is_drive_valid(drive.clone()){
        return FilesystemType::Exfat;
    }
    else if ntfs::is_drive_valid(drive.clone()){
        return FilesystemType::Ntfs;
    }
    else if ext4::is_drive_valid(drive.clone()){
        return FilesystemType::Ext4;
    }else {
        return FilesystemType::None;
    }
}

pub fn search(){
    todo!()
}

pub fn index_drives(drives: Vec<main::Drive>)->(Vec<main::File>, Vec<main::Directory>, u32){
    let mut items = (Vec::new(), Vec::new(), 0);
    let temp_drives = main::get_devices();
    for mut drive in drives.clone(){
        let mut found = false;
        for t in temp_drives.clone(){
            if t.mounted_at == drive.mounted_at{
                found = true;
                if drive.drive != t.drive{
                    drive.drive = t.drive;
                }
            }
        }
        if !found{
            continue;
        }
        let fs = check_drive_filesystem_type(drive.drive.clone());
        match fs{
            FilesystemType::None =>{}
            FilesystemType::Exfat => {
                let idx = items.1.len() as u32;
                let result = main::exfat::index(drive.drive, drive.mounted_at, drive.ignored_dirs, idx);
                if result.is_err(){
                    items.2 += result.err().unwrap();
                }else{
                    let (mut files, mut dir) = result.unwrap();
                    items.0.append(&mut files);
                    items.1.append(&mut dir);
                }
            }
            FilesystemType::Ext4 => {
                let idx = items.1.len() as u32;
                let result = main::ext4::index(drive.drive, drive.mounted_at, drive.ignored_dirs, idx);
                if result.is_err(){
                    items.2 += result.err().unwrap();
                }else{
                    let (mut files, mut dir) = result.unwrap();
                    items.0.append(&mut files);
                    items.1.append(&mut dir);
                }
            }
            FilesystemType::Ntfs => {
                let idx = items.1.len() as u32;
                let result = main::ntfs::index(drive.drive, drive.mounted_at, drive.ignored_dirs, idx);
                if result.is_err(){
                    items.2 += result.err().unwrap();
                }else{
                    let (mut files, mut dir) = result.unwrap();
                    items.0.append(&mut files);
                    items.1.append(&mut dir);
                }
            }
        }
    }
    items
}