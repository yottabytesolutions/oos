// SPDX-License-Identifier: GPL-2.0-or-later
//! Desktop readers have their own whole-media identity checks; they do not use the unit's SD controller.
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

#[cfg(any(target_os = "macos", test))]
pub(super) fn buffered_path(path: &Path) -> std::path::PathBuf {
    // Raw macOS disks require sector-aligned IO; the block alias permits the four-byte boot selector.
    match path.to_str().and_then(|s| s.strip_prefix("/dev/rdisk")) {
        Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => format!("/dev/disk{n}").into(),
        _ => path.to_path_buf(),
    }
}

pub(super) fn check(meta: &fs::Metadata, path: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let _ = path;
        if !meta.file_type().is_block_device() {
            return Err("host card must be a whole removable block device".into());
        }
        let number = meta.rdev();
        let sys = format!("/sys/dev/block/{}:{}", libc::major(number), libc::minor(number));
        check_linux(Path::new(&sys))
    }
    #[cfg(target_os = "macos")]
    {
        if !(meta.file_type().is_char_device() || meta.file_type().is_block_device()) {
            return Err("host card must be a whole removable disk".into());
        }
        let output = std::process::Command::new("/usr/sbin/diskutil")
            .args(["info", "-plist"])
            .arg(path)
            .output()
            .map_err(|e| format!("host card identity: {e}"))?;
        if !output.status.success() || fs::metadata(path).map_err(|e| e.to_string())?.rdev() != meta.rdev() {
            return Err("cannot identify the opened host card".into());
        }
        check_macos(&String::from_utf8_lossy(&output.stdout), path)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (meta, path);
        Err("host card readers are supported on Linux and macOS".into())
    }
}

#[cfg(any(target_os = "linux", test))]
fn check_linux(sys: &Path) -> Result<(), String> {
    if sys.join("partition").try_exists().map_err(|e| e.to_string())? {
        return Err("host card must be the whole device, not a partition".into());
    }
    if fs::read_to_string(sys.join("removable")).map_err(|e| e.to_string())?.trim() != "1" {
        return Err("host card is not removable: nothing written".into());
    }
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn check_macos(plist: &str, path: &Path) -> Result<(), String> {
    let value = |key| plist.split_once(&format!("<key>{key}</key>")).map(|(_, rest)| rest.trim_start());
    let boolean = |key| value(key).is_some_and(|v| v.starts_with("<true/>"));
    let string = |key, text| value(key).is_some_and(|v| v.starts_with(&format!("<string>{text}</string>")));
    let disk = path.to_str().and_then(|s| s.strip_prefix("/dev/")).unwrap_or("");
    let disk = disk.strip_prefix('r').unwrap_or(disk);
    let whole_name = disk.strip_prefix("disk").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    if !whole_name || !string("DeviceIdentifier", disk) || !boolean("Whole") {
        return Err("host card must be the identified whole disk, not a partition".into());
    }
    if !string("VirtualOrPhysical", "Physical")
        || !(string("BusProtocol", "USB") || string("BusProtocol", "Secure Digital"))
        || !(boolean("RemovableMedia") || value("Internal").is_some_and(|v| v.starts_with("<false/>")))
    {
        return Err("host card must be removable or external SD/USB media".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_reader_requires_whole_removable_media_without_a_board_controller() {
        let sys = std::env::temp_dir().join(format!("oos-host-reader-{}", std::process::id()));
        fs::create_dir(&sys).unwrap();
        assert!(check_linux(&sys).is_err());
        fs::write(sys.join("removable"), "1\n").unwrap();
        assert!(check_linux(&sys).is_ok());
        fs::write(sys.join("partition"), "1\n").unwrap();
        assert!(check_linux(&sys).is_err());
        fs::remove_file(sys.join("partition")).unwrap();
        for value in ["0\n", "", "unknown"] {
            fs::write(sys.join("removable"), value).unwrap();
            assert!(check_linux(&sys).is_err());
        }
        fs::remove_dir_all(sys).unwrap();
    }

    #[test]
    fn macos_reader_accepts_raw_disks_but_rejects_partitions_internal_and_virtual_media() {
        let plist = "<plist><dict><key>DeviceIdentifier</key><string>disk4</string>
            <key>Whole</key><true/><key>VirtualOrPhysical</key><string>Physical</string>
            <key>BusProtocol</key><string>USB</string><key>Internal</key><false/>
            <key>RemovableMedia</key><true/></dict></plist>";
        for path in ["/dev/rdisk4", "/dev/disk4"] {
            assert!(check_macos(plist, Path::new(path)).is_ok());
        }
        let path = Path::new("/dev/rdisk4");
        for bad in ["/dev/rdisk4s1", "/dev/rdisk5", "/dev/null", "/dev/rdisk", "disk4"] {
            assert!(check_macos(plist, Path::new(bad)).is_err());
        }
        for (from, to) in [
            ("<key>Whole</key><true/>", "<key>Whole</key><false/>"),
            ("Physical", "Virtual"),
            ("USB", "PCI-Express"),
            ("<key>DeviceIdentifier</key>", "<key>Other</key>"),
        ] {
            assert!(check_macos(&plist.replace(from, to), path).is_err());
        }
        let internal = plist.replace("<key>Internal</key><false/>", "<key>Internal</key><true/>");
        assert!(check_macos(&internal, path).is_ok(), "an internal SD reader can hold removable media");
        assert!(check_macos(&internal.replace("<key>RemovableMedia</key><true/>", ""), path).is_err());
        assert!(check_macos(&plist.replace("USB", "Secure Digital"), path).is_ok());
        assert!(check_macos("", path).is_err());
    }

    #[test]
    fn raw_macos_disks_use_the_buffered_alias_for_short_boot_writes() {
        assert_eq!(buffered_path(Path::new("/dev/rdisk4")), Path::new("/dev/disk4"));
        for path in ["/dev/rdisk4s1", "/dev/rdisk", "/dev/disk4", "/dev/sdb", "card.img"] {
            assert_eq!(buffered_path(Path::new(path)), Path::new(path));
        }
    }
}
