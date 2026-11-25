// SPDX-License-Identifier: GPL-2.0-or-later

use crate::system::durable::{replace_private, Disk, Files};
use std::io;
use std::path::Path;

const RAM: &str = "/run/oos-wlan/wpa_supplicant.conf";

pub(super) fn persist() -> io::Result<()> {
    persist_from(Path::new("/oos"), Path::new(RAM))
}

fn persist_from(data: &Path, ram: &Path) -> io::Result<()> {
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
    publish(&mut Disk, &directory.join("wpa_supplicant.conf"), &bytes)?;
    // The first successful save must also make the new wlan directory reachable.
    Disk.sync_dir(data)
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
}
