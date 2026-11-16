// SPDX-License-Identifier: GPL-2.0-or-later

//! Card-file exceptions follow file type, while hardware identity follows the opened device number.
use super::Card;
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

impl Card {
    /// Only the opened regular file can opt into simulated units, extra keys and direct test installs.
    pub fn is_test_file(&self) -> io::Result<bool> {
        Ok(self.f.metadata()?.is_file())
    }
}

/// Resolve aliases, then validate the opened object before any card reads or writes.
pub fn open_checked(path: &Path, write: bool) -> Result<Card, String> {
    let path = fs::canonicalize(path).map_err(|e| format!("card path: {e}"))?;
    let card = Card::open(&path, write).map_err(|e| format!("card open: {e}"))?;
    let meta = card.f.metadata().map_err(|e| format!("card metadata: {e}"))?;
    if meta.is_file() {
        return Ok(card);
    }
    if !meta.file_type().is_block_device() {
        return Err("card must be a regular test file or the board's SD block device".into());
    }
    #[cfg(target_os = "linux")]
    let number = meta.rdev();
    #[cfg(not(target_os = "linux"))]
    let number = meta.rdev() as libc::dev_t;
    let sys = format!("/sys/dev/block/{}:{}", libc::major(number), libc::minor(number));
    check_device(Path::new(&sys), crate::target::ACTIVE.card_controller)?;
    Ok(card)
}

/// Sysfs maps the opened device number to its controller on both stock Android and OOS.
fn check_device(sys: &Path, controller: &str) -> Result<(), String> {
    if sys.join("partition").try_exists().map_err(|e| format!("card partition: {e}"))? {
        return Err("card must be the whole SD block device: not touching a partition".into());
    }
    let dev = fs::canonicalize(sys.join("device")).map_err(|e| format!("SD card controller: {e}"))?;
    if !dev.components().any(|p| is_controller(&p.as_os_str().to_string_lossy(), controller)) {
        return Err(format!("card is not on the SD controller {controller}: not touching it"));
    }
    Ok(())
}

/// Sysfs names the controller node `<address>.<driver>`: `fe320000.mmc` under OOS, `fe320000.dwmmc` under stock.
fn is_controller(name: &str, controller: &str) -> bool {
    name.strip_prefix(controller).is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn opened_device_numbers_follow_the_controller_independent_of_mmc_names() {
        let root = std::env::temp_dir().join(format!("oos-card-controller-{}", std::process::id()));
        let controller = crate::target::ACTIVE.card_controller;
        // Real node names: OOS mainline and stock Android name the same controller differently.
        let hardware = root.join(format!("{controller}.mmc")).join("mmc_host/mmc0/mmc0:aaaa");
        let stock = root.join(format!("{controller}.dwmmc")).join("mmc_host/mmc1/mmc1:aaaa");
        fs::create_dir_all(&hardware).unwrap();
        fs::create_dir_all(&stock).unwrap();
        let by_number = root.join("dev/block");
        fs::create_dir_all(&by_number).unwrap();
        for (name, number, node) in [("mmcblk0", "179:0", &hardware), ("mmcblk1", "179:8", &stock)] {
            let disk = root.join("block").join(name);
            fs::create_dir_all(&disk).unwrap();
            symlink(node, disk.join("device")).unwrap();
            symlink(&disk, by_number.join(number)).unwrap();
            assert!(check_device(&by_number.join(number), controller).is_ok());
        }
        assert!(check_device(&by_number.join("179:16"), controller).is_err());
        let wrong = root.join(format!("{controller}-other"));
        fs::create_dir_all(&wrong).unwrap();
        let disk = root.join("block/mmcblk1");
        fs::remove_file(disk.join("device")).unwrap();
        symlink(&wrong, disk.join("device")).unwrap();
        assert!(check_device(&by_number.join("179:8"), controller).is_err());
        fs::remove_file(disk.join("device")).unwrap();
        symlink(&hardware, disk.join("device")).unwrap();
        fs::write(disk.join("partition"), "1\n").unwrap();
        assert!(check_device(&by_number.join("179:8"), controller).unwrap_err().contains("partition"));
        fs::remove_dir_all(root).unwrap();
    }
}
