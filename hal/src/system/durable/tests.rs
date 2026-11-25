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

    fn write(&mut self, path: &Path, bytes: &[u8], _: &OpenOptions) -> io::Result<()> {
        assert_ne!(path, self.path);
        assert_eq!(path.parent(), self.path.parent());
        self.staged = Some(bytes.to_vec());
        self.advance()
    }

    fn sync_file(&mut self, (): ()) -> io::Result<()> {
        self.synced = self.staged.take();
        self.advance()
    }

    fn rename(&mut self, _: &Path, to: &Path) -> io::Result<()> {
        assert_eq!(to, self.path);
        self.renamed = true;
        self.advance()
    }

    fn sync_dir(&mut self, dir: &Path) -> io::Result<()> {
        assert_eq!(Some(dir), self.path.parent());
        self.committed = true;
        self.advance()
    }
}

pub(crate) fn assert_cuts(path: &Path, new: &[u8], mut write: impl FnMut(&mut Cut) -> io::Result<()>) {
    for initial in [None, Some(OLD)] {
        for after in 1..=5 {
            let mut cut =
                Cut { path: path.into(), step: 0, after, staged: None, synced: None, renamed: false, committed: false };
            assert_eq!(write(&mut cut).is_err(), after <= 4);
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
