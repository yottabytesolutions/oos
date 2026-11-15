// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::collections::HashMap;

/// Models both inode data durability and directory-entry durability. A crash drops dirty data and
/// unsynced renames/unlinks, including an acknowledged rename whose directory sync did not finish.
#[derive(Clone, Default)]
struct Memory {
    files: HashMap<PathBuf, (usize, String)>,
    data: HashMap<usize, String>,
    names: HashMap<PathBuf, usize>,
    next: usize,
    calls: usize,
    cut: Option<usize>,
}

impl Memory {
    fn step(&mut self) -> io::Result<()> {
        self.calls += 1;
        if self.cut.is_some_and(|cut| self.calls > cut) {
            return Err(io::Error::other("power cut"));
        }
        Ok(())
    }

    fn crash(&self) -> Self {
        let mut out = self.clone();
        out.files = self
            .names
            .iter()
            .filter_map(|(path, id)| self.data.get(id).map(|text| (path.clone(), (*id, text.clone()))))
            .collect();
        out.cut = None;
        out.calls = 0;
        out
    }

    fn text(&self, path: &str) -> Option<String> {
        self.read(Path::new(path)).ok()
    }

    fn seed(&mut self, path: &str, text: &str) {
        durable::replace(self, Path::new(path), text).unwrap();
    }
}

impl Files for Memory {
    fn entries(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        Ok(self.files.keys().filter(|p| p.parent() == Some(path)).cloned().collect())
    }
    fn read(&self, path: &Path) -> io::Result<String> {
        self.files.get(path).map(|(_, text)| text.clone()).ok_or_else(|| io::ErrorKind::NotFound.into())
    }
    fn write(&mut self, path: &Path, text: &str) -> io::Result<()> {
        self.step()?;
        self.next += 1;
        self.files.insert(path.into(), (self.next, text.into()));
        Ok(())
    }
    fn sync_file(&mut self, path: &Path) -> io::Result<()> {
        self.step()?;
        let (id, text) = self.files.get(path).ok_or(io::ErrorKind::NotFound)?;
        self.data.insert(*id, text.clone());
        Ok(())
    }
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        self.step()?;
        let file = self.files.remove(from).ok_or(io::ErrorKind::NotFound)?;
        self.files.insert(to.into(), file);
        Ok(())
    }
    fn sync_dir(&mut self, path: &Path) -> io::Result<()> {
        self.step()?;
        self.names.retain(|p, _| p.parent() != Some(path));
        for (p, (id, _)) in &self.files {
            if p.parent() == Some(path) {
                self.names.insert(p.clone(), *id);
            }
        }
        Ok(())
    }
    fn remove(&mut self, path: &Path) -> io::Result<()> {
        self.step()?;
        self.files.remove(path);
        Ok(())
    }
}

fn fs() -> Memory {
    let mut fs = Memory::default();
    fs.seed("/data/oos-ui.setup", "last stable");
    fs.seed("/temporary/oos-ui.setup", "last stable");
    fs
}

fn start(fs: Memory, safe: bool) -> io::Result<Startup<Memory>> {
    Startup::start(fs, Path::new("/data"), Path::new("/temporary"), "boot:run\n".into(), safe)
}

#[test]
fn abort_restores_last_stable_setup() {
    let mut s = start(fs(), false).unwrap();
    assert_eq!(s.restore().unwrap().as_deref(), Some("last stable"));
    s.observe("dangerous".into(), Duration::ZERO).unwrap();
    s.observe("dangerous".into(), Duration::from_secs(59)).unwrap();
    let next = start(s.fs.crash(), false).unwrap();
    assert_eq!(next.restore().unwrap().as_deref(), Some("last stable"));
    assert_eq!(next.fs.text("/data/oos-ui.setup").as_deref(), Some("last stable"));
}

#[test]
fn each_candidate_gets_a_full_stability_interval() {
    let mut s = start(fs(), false).unwrap();
    s.observe("first".into(), Duration::ZERO).unwrap();
    s.observe("second".into(), Duration::from_secs(59)).unwrap();
    s.observe("second".into(), Duration::from_secs(118)).unwrap();
    assert_eq!(s.fs.text("/data/oos-ui.setup").as_deref(), Some("last stable"));
    s.observe("second".into(), Duration::from_secs(119)).unwrap();
    assert_eq!(s.fs.text("/data/oos-ui.setup").as_deref(), Some("second"));
    s.observe("third".into(), Duration::from_secs(120)).unwrap();
    s.observe("third".into(), Duration::from_secs(180)).unwrap();
    assert_eq!(s.fs.text("/data/oos-ui.setup").as_deref(), Some("third"));
}

#[test]
fn clean_power_off_persists_current_candidate() {
    let mut s = start(fs(), false).unwrap();
    s.power_off("new".into(), Duration::from_secs(1)).unwrap();
    assert_eq!(s.fs.text("/temporary/oos-ui.off").as_deref(), Some(""));
    let fs = s.fs.crash();
    assert_eq!(fs.text("/data/oos-ui.setup").as_deref(), Some("new"));
    assert_eq!(fs.text("/temporary/oos-ui.setup").as_deref(), Some("new"));
    assert_eq!(fs.text("/data/oos-ui.running"), None);
    assert_eq!(fs.text("/data/oos-ui.stable"), None);
}

#[test]
fn clean_stop_does_not_bless_unstable_settings() {
    let mut s = start(fs(), false).unwrap();
    s.clean("new".into(), Duration::from_secs(1), false).unwrap();
    let fs = s.fs.crash();
    assert_eq!(fs.text("/data/oos-ui.setup").as_deref(), Some("last stable"));
    assert_eq!(fs.text("/data/oos-ui.running"), None);
}

#[test]
fn safe_start_quarantines_setup_before_restore() {
    let s = start(fs(), true).unwrap();
    assert_eq!(s.restore().unwrap(), None);
    let fs = s.fs.crash();
    assert_eq!(fs.text("/data/oos-ui.setup.bad").as_deref(), Some("last stable"));
    assert_eq!(fs.text("/data/oos-ui.setup"), None);
    assert_eq!(fs.text("/temporary/oos-ui.setup"), None);
    assert!(fs.text("/data/oos-ui.running").is_some());
}

#[test]
fn cuts_keep_old_or_new_file() {
    for path in ["/data/oos-ui.running", "/data/oos-ui.stable", "/data/oos-ui.setup", "/temporary/oos-ui.setup"] {
        for cut in 0..=4 {
            let mut fs = fs();
            fs.seed(path, "old");
            fs.cut = Some(fs.calls + cut);
            let result = durable::replace(&mut fs, Path::new(path), "new");
            let after = fs.crash().text(path).unwrap();
            assert!(after == "old" || after == "new", "{} after {}", path, cut);
            assert_eq!(result.is_ok(), cut == 4);
            if result.is_ok() {
                assert_eq!(after, "new");
            }
        }
    }
}

#[test]
fn cuts_never_mark_an_unwritten_setup_stable() {
    for cut in 0..=14 {
        let mut s = start(fs(), false).unwrap();
        s.observe("new".into(), Duration::ZERO).unwrap();
        s.fs.cut = Some(s.fs.calls + cut);
        let result = s.observe("new".into(), Duration::from_secs(60));
        let fs = s.fs.crash();
        if fs.text("/data/oos-ui.stable").is_some() {
            assert_eq!(fs.text("/data/oos-ui.setup").as_deref(), Some("new"), "cut {}", cut);
            assert_eq!(fs.text("/temporary/oos-ui.setup").as_deref(), Some("new"), "cut {}", cut);
        }
        if result.is_ok() {
            assert_eq!(fs.text("/data/oos-ui.running"), None);
        }
    }
}

#[derive(Clone)]
struct Shared(std::rc::Rc<std::cell::RefCell<Memory>>);

impl Files for Shared {
    fn entries(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        self.0.borrow().entries(path)
    }
    fn read(&self, p: &Path) -> io::Result<String> {
        self.0.borrow().read(p)
    }
    fn write(&mut self, p: &Path, text: &str) -> io::Result<()> {
        self.0.borrow_mut().write(p, text)
    }
    fn sync_file(&mut self, p: &Path) -> io::Result<()> {
        self.0.borrow_mut().sync_file(p)
    }
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        self.0.borrow_mut().rename(from, to)
    }
    fn sync_dir(&mut self, p: &Path) -> io::Result<()> {
        self.0.borrow_mut().sync_dir(p)
    }
    fn remove(&mut self, p: &Path) -> io::Result<()> {
        self.0.borrow_mut().remove(p)
    }
}

#[test]
fn cuts_during_start_and_quarantine_preserve_recovery() {
    for safe in [false, true] {
        for cut in 0..=10 {
            let mut before = fs();
            before.seed("/data/oos-ui.stable", "previous run");
            before.cut = Some(before.calls + cut);
            let shared = Shared(std::rc::Rc::new(std::cell::RefCell::new(before)));
            let _ = Startup::start(shared.clone(), Path::new("/data"), Path::new("/temporary"), "new run".into(), safe);
            let fs = shared.0.borrow().crash();
            assert!(fs.text("/data/oos-ui.setup").is_some() || fs.text("/data/oos-ui.setup.bad").is_some());
            if fs.text("/data/oos-ui.stable").is_none() {
                assert!(fs.text("/data/oos-ui.running").is_some(), "cut {}", cut);
            }
            let next = start(fs, safe).unwrap();
            assert_eq!(next.restore().unwrap().is_none(), safe);
        }
    }
}

#[test]
fn cuts_during_clean_exit_never_remove_markers_before_power_off_setup() {
    for cut in 0..=12 {
        let mut s = start(fs(), false).unwrap();
        s.fs.cut = Some(s.fs.calls + cut);
        let result = s.clean("new".into(), Duration::ZERO, true);
        let fs = s.fs.crash();
        if fs.text("/data/oos-ui.running").is_none() {
            assert_eq!(fs.text("/data/oos-ui.setup").as_deref(), Some("new"), "cut {}", cut);
            assert_eq!(fs.text("/temporary/oos-ui.setup").as_deref(), Some("new"), "cut {}", cut);
        }
        if result.is_ok() {
            assert_eq!(fs.text("/data/oos-ui.running"), None);
        }
    }
}

/// Directory operations can reach disk before their directory fsync too. Model
/// both outcomes, with only file data that passed its own fsync surviving.
fn cut(fs: &Memory, namespace_reached_disk: bool) -> Memory {
    let mut disk = fs.clone();
    if namespace_reached_disk {
        disk.names = disk.files.iter().map(|(path, (id, _))| (path.clone(), *id)).collect();
    }
    disk.crash()
}

#[test]
fn every_promotion_cut_retries_with_old_or_new_complete_setups() {
    for namespace in [false, true] {
        for boundary in 0..=14 {
            let mut s = start(fs(), false).unwrap();
            s.observe("first stable".into(), Duration::ZERO).unwrap();
            s.observe("first stable".into(), STABLE).unwrap();
            s.observe("second stable".into(), STABLE + Duration::from_secs(1)).unwrap();
            s.fs.cut = Some(s.fs.calls + boundary);
            let _ = s.observe("second stable".into(), STABLE * 2 + Duration::from_secs(1));
            let disk = cut(&s.fs, namespace);
            for path in ["/data/oos-ui.setup", "/temporary/oos-ui.setup"] {
                let text = disk.text(path).unwrap();
                assert!(text == "first stable" || text == "second stable", "cut {}", boundary);
            }
            let mut retry = start(disk, false).unwrap();
            retry.observe("third".into(), Duration::ZERO).unwrap();
            retry.observe("third".into(), STABLE).unwrap();
            let disk = retry.fs.crash();
            assert_eq!(disk.text("/data/oos-ui.setup").as_deref(), Some("third"));
            assert_eq!(disk.text("/temporary/oos-ui.setup").as_deref(), Some("third"));
        }
    }
}

#[test]
fn every_power_off_cut_can_retry_without_blessing_partial_files() {
    for namespace in [false, true] {
        for boundary in 0..=13 {
            let mut s = start(fs(), false).unwrap();
            s.fs.cut = Some(s.fs.calls + boundary);
            let result = s.power_off("new".into(), Duration::ZERO);
            let disk = cut(&s.fs, namespace);
            if disk.text("/data/oos-ui.running").is_none() || result.is_ok() {
                assert_eq!(disk.text("/data/oos-ui.setup").as_deref(), Some("new"));
                assert_eq!(disk.text("/temporary/oos-ui.setup").as_deref(), Some("new"));
            }
            let mut retry = start(disk, false).unwrap();
            retry.power_off("new".into(), Duration::ZERO).unwrap();
            assert_eq!(retry.fs.crash().text("/data/oos-ui.running"), None);
            assert_eq!(retry.fs.text("/temporary/oos-ui.off").as_deref(), Some(""));
        }
    }
}

#[test]
fn every_quarantine_cut_restarts_safe_with_both_directory_outcomes() {
    for namespace in [false, true] {
        for boundary in 0..=10 {
            let mut before = fs();
            before.cut = Some(before.calls + boundary);
            let shared = Shared(std::rc::Rc::new(std::cell::RefCell::new(before)));
            let _ = Startup::start(shared.clone(), Path::new("/data"), Path::new("/temporary"), "new".into(), true);
            let mut retry = start(cut(&shared.0.borrow(), namespace), true).unwrap();
            assert_eq!(retry.restore().unwrap(), None);
            assert_eq!(retry.fs.text("/data/oos-ui.setup"), None);
            assert_eq!(retry.fs.text("/temporary/oos-ui.setup"), None);
            assert_eq!(retry.fs.text("/data/oos-ui.setup.bad").as_deref(), Some("last stable"));
            retry.observe("defaults".into(), Duration::ZERO).unwrap();
            retry.observe("defaults".into(), STABLE).unwrap();
        }
    }
}

mod review;
