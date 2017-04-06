#[macro_use]
extern crate log;

use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};
    use std::path::{Path, PathBuf};
    use super::FsEntry;
    #[test]
    fn test_parser() {
        let expected_results =
            vec![FsEntry {
                     fs_spec: "/dev/mapper/xubuntu--vg--ssd-root".to_string(),
                     mountpoint: PathBuf::from("/"),
                     vfs_type: "ext4".to_string(),
                     mount_options: vec!["noatime".to_string(), "errors=remount-ro".to_string()],
                     dump: false,
                     fsck_order: 1,
                 },
                 FsEntry {
                     fs_spec: "UUID=378f3c86-b21a-4172-832d-e2b3d4bc7511".to_string(),
                     mountpoint: PathBuf::from("/boot"),
                     vfs_type: "ext2".to_string(),
                     mount_options: vec!["defaults".to_string()],
                     dump: false,
                     fsck_order: 2,
                 },
                 FsEntry {
                     fs_spec: "/dev/mapper/xubuntu--vg--ssd-swap_1".to_string(),
                     mountpoint: PathBuf::from("none"),
                     vfs_type: "swap".to_string(),
                     mount_options: vec!["sw".to_string()],
                     dump: false,
                     fsck_order: 0,
                 },
                 FsEntry {
                     fs_spec: "UUID=be8a49b9-91a3-48df-b91b-20a0b409ba0f".to_string(),
                     mountpoint: PathBuf::from("/mnt/raid"),
                     vfs_type: "ext4".to_string(),
                     mount_options: vec!["errors=remount-ro".to_string(), "user".to_string()],
                     dump: false,
                     fsck_order: 1,
                 }];
        let input = r#"
# /etc/fstab: static file system information.
#
# Use 'blkid' to print the universally unique identifier for a
# device; this may be used with UUID= as a more robust way to name devices
# that works even if disks are added and removed. See fstab(5).
#
# <file system> <mount point>   <type>  <options>       <dump>  <pass>
/dev/mapper/xubuntu--vg--ssd-root /               ext4    noatime,errors=remount-ro 0       1
# /boot was on /dev/sda1 during installation
UUID=378f3c86-b21a-4172-832d-e2b3d4bc7511 /boot           ext2    defaults        0       2
/dev/mapper/xubuntu--vg--ssd-swap_1 none            swap    sw              0       0
UUID=be8a49b9-91a3-48df-b91b-20a0b409ba0f /mnt/raid ext4 errors=remount-ro,user 0 1
# tmpfs /tmp tmpfs rw,nosuid,nodev
"#;
        //let bytes = input.as_bytes();
        //let results = super::parse_fstab("/etc/fstab").unwrap();
        //println!("Result: {:?}", results);
        //assert_eq!(results, expected_results);

        //let bytes_written = super::add_entry(expected_results[1].clone(), Path::new("/tmp/fstab"))
        //    .unwrap();
        //println!("Wrote: {}", bytes_written);
    }
}

/// For help with what these fields mean consult: `man fstab` on linux.
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct FsEntry {
    /// The device identifier
    pub fs_spec: String,
    /// The mount point
    pub mountpoint: PathBuf,
    /// Which filesystem type it is
    pub vfs_type: String,
    /// Mount options to use
    pub mount_options: Vec<String>,
    /// This field is used by dump(8) to determine which filesystems need to be dumped
    pub dump: bool,
    /// This field is used by fsck(8) to determine the order in which filesystem checks
    /// are done at boot time.
    pub fsck_order: u16,
}

/// Takes the location to the fstab and parses it.  On linux variants
/// this is usually /etc/fstab.  On SVR4 systems store block devices and
/// mount point information in /etc/vfstab file. AIX stores block device
/// and mount points information in /etc/filesystems file.
pub fn parse_fstab(file: &Path) -> Result<Vec<FsEntry>, Error> {
    let mut entries: Vec<FsEntry> = Vec::new();

    let mut file = File::open(file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    for line in contents.lines() {
        if line.starts_with("#") {
            trace!("Skipping commented line: {}", line);
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 6 {
            debug!("Unknown fstab entry: {}", line);
            continue;
        }
        let fsck_order =
            u16::from_str(parts[5]).map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
        entries.push(FsEntry {
                         fs_spec: parts[0].to_string(),
                         mountpoint: PathBuf::from(parts[1]),
                         vfs_type: parts[2].to_string(),
                         mount_options: parts[3].split(",").map(|s| s.to_string()).collect(),
                         dump: if parts[4] == "0" { false } else { true },
                         fsck_order: fsck_order,
                     })
    }
    Ok(entries)
}

fn save_fstab(entries: &Vec<FsEntry>, file: &Path) -> Result<usize, Error> {
    let mut file = File::create(file)?;
    let mut bytes_written: usize = 0;
    for entry in entries {
        bytes_written += file.write(&format!("{spec} {mount} {vfs} {options} {dump} {fsck}\n",
                                             spec = entry.fs_spec,
                                             mount = entry.mountpoint.display(),
                                             vfs = entry.vfs_type,
                                             options = entry.mount_options.join(","),
                                             dump = if entry.dump { "1" } else { "0" },
                                             fsck = entry.fsck_order)
                                             .as_bytes())?;
    }
    file.flush()?;
    debug!("Wrote {} bytes to fstab", bytes_written);
    Ok(bytes_written)
}

/// Add a new entry to the fstab.  If the fstab previously did not contain this entry
/// then true is returned.  Otherwise it will return false indicating it has been updated
pub fn add_entry(entry: FsEntry, file: &Path) -> Result<bool, Error> {
    let mut entries = parse_fstab(&file)?;

    let position = entries.iter().position(|e| e == &entry);
    if let Some(pos) = position {
        debug!("Removing {} from fstab entries", pos);
        entries.remove(pos);
    }
    entries.push(entry);
    save_fstab(&mut entries, &file)?;

    match position {
        Some(_) => Ok(false),
        None => Ok(true),
    }
}

/// Remove the fstab entry that corresponds to the spec given.  IE: first fields match
/// Returns true if the value was present in the fstab.
pub fn remove_entry(spec: String, file: &Path) -> Result<bool, Error> {
    let mut entries = parse_fstab(&file)?;
    let position = entries.iter().position(|e| e.fs_spec == spec);

    match position {
        Some(pos) => {
            debug!("Removing {} from fstab entries", pos);
            entries.remove(pos);
            save_fstab(&mut entries, &file)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
