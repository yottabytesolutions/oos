// SPDX-License-Identifier: GPL-2.0-or-later

use crate::system::durable::{replace_private, Disk, Files};
use std::io;
use std::path::Path;

const RAM: &str = "/run/oos-wlan/wpa_supplicant.conf";
const REJECTED: &str = "rejected";

pub(super) fn persist() -> io::Result<()> {
    persist_from(Path::new("/oos"), Path::new(RAM))
}

fn persist_from(data: &Path, ram: &Path) -> io::Result<()> {
    crate::system::check_data_write(data)?;
    // Use the same data-area readiness check as the WLAN supervisor.
    if !data.join("setups").is_dir() {
        return Ok(());
    }
    let bytes = std::fs::read(ram)?;
    let directory = data.join("wlan");
    match std::fs::create_dir(&directory) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    let saved = directory.join("wpa_supplicant.conf");
    preserve_rejected(ram, &saved)?;
    publish(&mut Disk, &saved, &bytes)?;
    // The first successful save must also make the new wlan directory reachable.
    Disk.sync_dir(data)
}

fn preserve_rejected(ram: &Path, saved: &Path) -> io::Result<()> {
    let marker = ram.with_added_extension(REJECTED);
    if marker.try_exists()? {
        let backup = saved.with_added_extension(REJECTED);
        publish(&mut Disk, &backup, &std::fs::read(saved)?)?;
        eprintln!("Rejected WLAN configuration retained at {}", backup.display());
        // The original is protected from this point on, even when publishing the new list fails.
        std::fs::remove_file(marker)?;
    }
    Ok(())
}

fn publish(files: &mut impl Files, saved: &Path, bytes: &[u8]) -> io::Result<()> {
    replace_private(files, saved, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &[u8] =
        b"ctrl_interface=/run/wpa_supplicant\nupdate_config=1\nnetwork={\n\tssid=74657374\n\tkey_mgmt=NONE\n}\n";

    #[test]
    fn closed_data_refuses_wlan_configuration() {
        use crate::system::admission_test::{assert_refused, with_closed_data};
        with_closed_data(module_path!(), "closed_data_refuses_wlan_configuration", |data, ram| {
            std::fs::create_dir(data.join("setups")).unwrap();
            let source = ram.join("wpa_supplicant.conf");
            std::fs::write(&source, CONFIG).unwrap();
            assert_refused(persist_from(data, &source).unwrap_err());
            assert!(!data.join("wlan").exists());
        });
    }

    #[test]
    fn a_cut_keeps_a_complete_saved_configuration() {
        let path = Path::new("wlan/wpa_supplicant.conf");
        crate::system::durable::tests::assert_cuts(path, CONFIG, |files| publish(files, path, CONFIG));
    }

    #[test]
    fn save_requires_data_area_and_creates_a_private_copy() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("oos-wlan-save-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let ram = dir.join("ram.conf");
        let path = dir.join("wlan/wpa_supplicant.conf");
        persist_from(&dir, &ram).unwrap();
        assert!(!path.exists());
        std::fs::create_dir(dir.join("setups")).unwrap();
        std::fs::write(&ram, CONFIG).unwrap();
        persist_from(&dir, &ram).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), CONFIG);
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        std::fs::remove_file(&ram).unwrap();
        assert!(persist_from(&dir, &ram).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), CONFIG);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn supervisor_and_ui_agree_on_ram_paths() {
        let supervisor = include_str!("../../../rootfs/initramfs/bin/oos-wlan");
        assert!(supervisor.lines().any(|line| line == format!("CONF={RAM}")));
        assert!(supervisor.lines().any(|line| line == format!("REJECTED=$CONF.{REJECTED}")));
    }

    #[test]
    fn fallback_save_keeps_one_private_backup_before_replacing_the_saved_file() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("oos-wlan-fallback-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        fs::create_dir(dir.join("setups")).unwrap();
        fs::create_dir(dir.join("wlan")).unwrap();
        let saved = dir.join("wlan/wpa_supplicant.conf");
        let backup = saved.with_added_extension("rejected");
        let ram = dir.join("ram.conf");
        let marker = ram.with_added_extension("rejected");
        let rejected = b"invalid saved configuration";
        fs::write(&saved, rejected).unwrap();
        fs::write(&ram, CONFIG).unwrap();
        fs::write(&marker, b"").unwrap();
        fs::create_dir(&backup).unwrap();
        assert!(persist_from(&dir, &ram).is_err());
        assert_eq!(fs::read(&saved).unwrap(), rejected);
        assert!(marker.exists());
        fs::remove_dir(&backup).unwrap();
        fs::write(&backup, b"earlier rejected configuration").unwrap();
        persist_from(&dir, &ram).unwrap();
        assert_eq!(fs::read(&saved).unwrap(), CONFIG);
        assert_eq!(fs::read(&backup).unwrap(), rejected);
        assert_eq!(fs::metadata(&backup).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(!marker.exists());
        fs::write(&ram, b"next save").unwrap();
        persist_from(&dir, &ram).unwrap();
        assert_eq!(fs::read(&saved).unwrap(), b"next save");
        assert_eq!(fs::read(&backup).unwrap(), rejected);
        fs::remove_dir_all(dir).unwrap();
    }
}
