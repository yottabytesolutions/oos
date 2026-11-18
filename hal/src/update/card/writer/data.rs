// SPDX-License-Identifier: GPL-2.0-or-later
//! Filesystem tools work on a regular image; only the checked writer touches the card.
use super::*;
use std::fs::{self, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Command;

struct Image(std::path::PathBuf);

impl Drop for Image {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl Card {
    /// Format an empty OOS data area or repair its ext filesystem before it is mounted.
    pub fn prepare_data(&mut self) -> Result<(), String> {
        self.repair_data(|path, empty| {
            if empty {
                let status = Command::new("mke2fs")
                    .args(["-F", "-L", "oosdata"])
                    .arg(path)
                    .status()
                    .map_err(|e| e.to_string())?;
                if !status.success() {
                    return Err("formatting OOS data failed".into());
                }
            }
            let status = Command::new("e2fsck").arg("-p").arg(path).status().map_err(|e| e.to_string())?;
            if !matches!(status.code(), Some(0 | 1)) {
                return Err(format!("OOS data repair failed: {status}"));
            }
            Ok(())
        })
    }

    fn repair_data(&mut self, repair: impl FnOnce(&Path, bool) -> Result<(), String>) -> Result<(), String> {
        self.check_layout()?;
        let len = (crate::target::ACTIVE.layout.oosdata_sectors * SECTOR) as usize;
        let before = self.read(OOSDATA_LBA, len).map_err(|e| e.to_string())?;
        let empty = before.iter().all(|&b| b == 0);
        if !empty && before.get(1080..1082) != Some(&[0x53, 0xef]) {
            return Err("OOS data is neither ext nor empty: nothing written".into());
        }
        let path = std::env::temp_dir().join(format!("oos-data-{}.img", std::process::id()));
        let mut file =
            OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(|e| e.to_string())?;
        let image = Image(path);
        file.write_all(&before).map_err(|e| e.to_string())?;
        drop(file);
        repair(&image.0, empty)?;
        let after = fs::read(&image.0).map_err(|e| e.to_string())?;
        if after.len() != len {
            return Err("filesystem tool changed OOS data size: nothing written".into());
        }
        let sector = SECTOR as usize;
        let mut pending = None;
        let pairs = before.chunks_exact(sector).zip(after.chunks_exact(sector));
        for (index, (old, new)) in pairs.chain(std::iter::once((&[][..], &[][..]))).enumerate() {
            if old != new {
                pending.get_or_insert(index * sector);
            } else if let Some(start) = pending.take() {
                self.write_checked("backup", OOSDATA_LBA * SECTOR + start as u64, &after[start..index * sector])?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn repair_image_never_changes_other_areas_and_failures_do_not_write() {
        let path = std::env::temp_dir().join(format!("oos-data-card-{}", std::process::id()));
        let mut file = OpenOptions::new().write(true).read(true).create_new(true).open(&path).unwrap();
        let _cleanup = Image(path.clone());
        let end = (OOSDATA_LBA + crate::target::ACTIVE.layout.oosdata_sectors) * SECTOR;
        file.set_len(end + 512).unwrap();
        file.seek(SeekFrom::Start(crate::update::card::PARM_LBA * SECTOR)).unwrap();
        file.write_all(crate::target::ACTIVE.parm_marks.join(" ").as_bytes()).unwrap();
        file.seek(SeekFrom::Start(OOSDATA_LBA * SECTOR - 1)).unwrap();
        file.write_all(&[91]).unwrap();
        file.seek(SeekFrom::Start(end)).unwrap();
        file.write_all(&[92]).unwrap();
        let mut card = Card::open(&path, true).unwrap();
        assert!(card
            .repair_data(|image, _| {
                fs::write(image, [99]).unwrap();
                Err("failed repair".into())
            })
            .is_err());
        assert!(card.read(OOSDATA_LBA, 1082).unwrap().iter().all(|&b| b == 0));
        card.repair_data(|image, empty| {
            assert!(empty);
            let mut f = OpenOptions::new().write(true).open(image).unwrap();
            f.write_all(&[17]).unwrap();
            f.seek(SeekFrom::Start(1080)).unwrap();
            f.write_all(&[0x53, 0xef]).unwrap();
            f.seek(SeekFrom::End(-1)).unwrap();
            f.write_all(&[18]).unwrap();
            Ok(())
        })
        .unwrap();
        let data = card.read(OOSDATA_LBA, 1082).unwrap();
        assert_eq!(data[0], 17);
        assert_eq!(&data[1080..], &[0x53, 0xef]);
        assert_eq!(card.read(OOSDATA_LBA + crate::target::ACTIVE.layout.oosdata_sectors - 1, 512).unwrap()[511], 18);
        card.repair_data(|_, empty| {
            assert!(!empty);
            Ok(())
        })
        .unwrap();
        assert!(card
            .repair_data(|image, _| {
                fs::write(image, [0]).unwrap();
                Ok(())
            })
            .is_err());
        for (offset, value) in [(OOSDATA_LBA * SECTOR - 1, 91), (end, 92)] {
            let mut byte = [0];
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.read_exact(&mut byte).unwrap();
            assert_eq!(byte, [value]);
        }
    }
}
