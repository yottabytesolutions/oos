// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const OLD: &[u8] = b"previous complete file";

// A crash can discard the rename until the directory is synced. A published inode
// keeps only the bytes synced before the crash, even if the rename itself survives.
pub(crate) struct Cut {
    path: std::path::PathBuf,
    step: usize,
    after: usize,
    staged: Option<Vec<u8>>,
    synced: Option<Vec<u8>>,
    renamed: bool,
    committed: bool,
    temporary: bool,
}

impl Cut {
    fn advance(&mut self) -> io::Result<()> {
        self.step += 1;
        if self.step == self.after {
            return Err(io::Error::other("power cut"));
        }
        Ok(())
    }
}

impl Files for Cut {
    type Staged = ();

    fn create(&mut self, path: &Path, _: &OpenOptions) -> io::Result<()> {
        assert_ne!(path, self.path);
        assert_eq!(path.parent(), self.path.parent());
        assert!(!self.temporary);
        self.temporary = true;
        Ok(())
    }

    fn write(&mut self, (): &mut (), bytes: &[u8]) -> io::Result<()> {
        let result = self.advance();
        self.staged = Some(if result.is_ok() { bytes.to_vec() } else { bytes[..1].to_vec() });
        result
    }

    fn sync_file(&mut self, (): ()) -> io::Result<()> {
        self.synced = self.staged.take();
        self.advance()
    }

    fn rename(&mut self, _: &Path, to: &Path) -> io::Result<()> {
        assert_eq!(to, self.path);
        self.renamed = true;
        self.temporary = false;
        self.advance()
    }

    fn sync_dir(&mut self, dir: &Path) -> io::Result<()> {
        assert_eq!(Some(dir), self.path.parent());
        self.committed = true;
        self.advance()
    }

    fn remove(&mut self, path: &Path) -> io::Result<()> {
        assert_eq!(path, self.path.with_added_extension("new"));
        self.temporary = false;
        Ok(())
    }
}

pub(crate) fn assert_cuts(path: &Path, new: &[u8], mut write: impl FnMut(&mut Cut) -> io::Result<()>) {
    for initial in [None, Some(OLD)] {
        for after in 1..=5 {
            let mut cut = Cut {
                path: path.into(),
                step: 0,
                after,
                staged: None,
                synced: None,
                renamed: false,
                committed: false,
                temporary: true,
            };
            assert_eq!(write(&mut cut).is_err(), after <= 4);
            assert!(!cut.temporary);
            assert_eq!(cut.step, after.min(4));
            for rename_survives in [false, true] {
                let recovered =
                    if cut.committed || (rename_survives && cut.renamed) { cut.synced.as_deref() } else { initial };
                let expected = if after >= 4 || (after == 3 && rename_survives) { Some(new) } else { initial };
                assert_eq!(recovered, expected, "cut after step {after}");
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn private_replacement_discards_a_loose_stale_file_and_cleans_up_after_failed_rename() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("oos-durable-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("saved");
    let staged = path.with_added_extension("new");
    fs::write(&staged, b"stale").unwrap();
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o666)).unwrap();
    replace_private(&mut Disk, &path, b"complete").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"complete");
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(!staged.exists());

    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(replace_private(&mut Disk, &path, b"another copy").is_err());
    assert!(path.is_dir());
    assert!(!staged.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[derive(Default)]
struct Removal {
    present: bool,
    synced: Vec<std::path::PathBuf>,
}

impl Files for Removal {
    type Staged = ();

    fn create(&mut self, _: &Path, _: &OpenOptions) -> io::Result<()> {
        unreachable!("a removal stages nothing")
    }

    fn write(&mut self, (): &mut (), _: &[u8]) -> io::Result<()> {
        unreachable!("a removal writes nothing")
    }

    fn sync_file(&mut self, (): ()) -> io::Result<()> {
        unreachable!("a removal syncs no file")
    }

    fn rename(&mut self, _: &Path, _: &Path) -> io::Result<()> {
        unreachable!("a removal renames nothing")
    }

    fn sync_dir(&mut self, dir: &Path) -> io::Result<()> {
        self.synced.push(dir.to_path_buf());
        Ok(())
    }

    fn remove(&mut self, _: &Path) -> io::Result<()> {
        if std::mem::take(&mut self.present) {
            Ok(())
        } else {
            Err(io::ErrorKind::NotFound.into())
        }
    }
}

#[test]
fn a_removal_syncs_the_directory_so_the_file_cannot_reappear() {
    let mut files = Removal { present: true, ..Removal::default() };
    durable_remove(&mut files, Path::new("/data/update/kexec-restart")).unwrap();
    assert!(!files.present);
    assert_eq!(files.synced, [Path::new("/data/update")]);
}

#[test]
fn removing_a_missing_file_succeeds_without_a_sync() {
    let mut files = Removal::default();
    durable_remove(&mut files, Path::new("/data/update/kexec-restart")).unwrap();
    assert!(files.synced.is_empty());
}
