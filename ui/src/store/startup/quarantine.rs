// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

impl<F: Files> Startup<F> {
    pub(super) fn quarantine(&mut self) -> io::Result<()> {
        let setup = self.paths.data.join("oos-ui.setup");
        match self.fs.read(&setup) {
            Ok(_) => self.keep_quarantine(&setup)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        durable::remove(&mut self.fs, &self.paths.temporary.join("oos-ui.setup"))
    }

    fn keep_quarantine(&mut self, setup: &Path) -> io::Result<()> {
        let first = self.paths.data.join("oos-ui.setup.bad");
        let target = match self.fs.read(&first) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => first.clone(),
            Ok(_) => {
                let mut stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(io::Error::other)?
                    .as_nanos();
                loop {
                    let target = self.paths.data.join(format!("oos-ui.setup.bad.{}", stamp));
                    match self.fs.read(&target) {
                        Err(e) if e.kind() == io::ErrorKind::NotFound => break target,
                        Ok(_) => stamp += 1,
                        Err(e) => return Err(e),
                    }
                }
            }
            Err(e) => return Err(e),
        };
        // Prune before rename: even an interrupted safe start retains at most three copies.
        let mut later: Vec<_> = self
            .fs
            .entries(&self.paths.data)?
            .into_iter()
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("oos-ui.setup.bad.")))
            .collect();
        later.sort_by_key(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix("oos-ui.setup.bad."))
                .and_then(|n| n.parse::<u128>().ok())
                .unwrap_or(0)
        });
        let excess = later.len().saturating_sub(1);
        for path in later.into_iter().take(excess) {
            durable::remove(&mut self.fs, &path)?;
        }
        self.fs.rename(setup, &target)?;
        self.fs.sync_dir(&self.paths.data)
    }
}
