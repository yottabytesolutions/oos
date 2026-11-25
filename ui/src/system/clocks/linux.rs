// SPDX-License-Identifier: GPL-2.0-or-later
//! Linux cpufreq/devfreq adapter. Discovery never assumes a devfreq index.
use super::{floor, Config, Controller, Port};
use std::io;
use std::path::{Path, PathBuf};

pub(super) struct Sysfs(pub(super) PathBuf);
impl Port for Sysfs {
    fn read(&self, path: &str) -> io::Result<String> {
        std::fs::read_to_string(self.0.join(path))
    }
    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        std::fs::read_dir(self.0.join(path))?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect()
    }
    fn read_link(&self, path: &str) -> io::Result<PathBuf> {
        std::fs::read_link(self.0.join(path))
    }
    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        std::fs::write(self.0.join(path), value.to_string())
    }
}

pub(super) fn controller(root: &Path, journal: &Path, config: Config) -> Controller {
    let port = Sysfs(root.into());
    // RK3399's GPU node is named after the device, independent of class entry order.
    let gpu = floor::gpu_node(&port).ok().flatten();
    Controller::new(Box::new(port), config, gpu.as_deref(), journal.into())
}
