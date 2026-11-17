// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn storage_errors_disable_persistence_in_normal_and_safe_start() {
    for safe in [false, true] {
        let mut disk = fs();
        disk.cut = Some(disk.calls);
        let mut state = start(disk, safe).expect("storage failure must allow the UI to start");
        assert_eq!(state.restore().unwrap(), None);
        let calls = state.fs.calls;
        state.observe("defaults".into(), Duration::ZERO).unwrap();
        state.observe("defaults".into(), STABLE).unwrap();
        assert_eq!(state.fs.calls, calls, "disabled state must not attempt promotion");
    }
}

#[test]
fn persist_failure_backs_off_and_reports_once_until_recovery() {
    let mut s = start(fs(), false).unwrap();
    s.observe("new".into(), Duration::ZERO).unwrap();
    s.fs.cut = Some(s.fs.calls);
    assert!(s.observe("new".into(), STABLE).is_err());
    let calls = s.fs.calls;
    for ms in 1..1000 {
        assert!(s.observe("new".into(), STABLE + Duration::from_millis(ms)).is_ok());
    }
    assert_eq!(s.fs.calls, calls, "no writes in the first backoff second");
    assert!(s.observe("new".into(), STABLE + Duration::from_secs(1)).is_ok());
    assert_eq!(s.fs.calls, calls + 1);
    s.fs.cut = None;
    s.observe("new".into(), STABLE + Duration::from_secs(3)).unwrap();
    assert_eq!(s.fs.text("/data/oos-ui.setup").as_deref(), Some("new"));
    s.observe("next".into(), Duration::from_secs(64)).unwrap();
    s.fs.cut = Some(s.fs.calls);
    assert!(s.observe("next".into(), Duration::from_secs(124)).is_err(), "recovery re-arms logging");
}

#[test]
fn failed_power_off_save_still_stops_the_supervisor() {
    let shared = Shared(std::rc::Rc::new(std::cell::RefCell::new(fs())));
    let mut s = Startup::start(
        DataFailure { disk: shared.clone(), failure: None },
        Path::new("/data"),
        Path::new("/temporary"),
        "run".into(),
        false,
    )
    .unwrap();
    s.fs.failure = Some(libc::ENOSPC);
    assert!(s.power_off("new".into(), Duration::ZERO).is_err());
    assert_eq!(shared.0.borrow().text("/temporary/oos-ui.off").as_deref(), Some(""));
}

struct DataFailure {
    disk: Shared,
    failure: Option<i32>,
}
impl Files for DataFailure {
    fn prepare(&mut self, p: &Path) -> io::Result<()> {
        self.disk.prepare(p)
    }
    fn entries(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        self.disk.entries(path)
    }
    fn read(&self, p: &Path) -> io::Result<String> {
        self.disk.read(p)
    }
    fn read_limited(&self, p: &Path, limit: usize) -> io::Result<String> {
        self.disk.read_limited(p, limit)
    }
    fn write(&mut self, p: &Path, t: &str) -> io::Result<()> {
        if p.starts_with("/data") {
            if let Some(code) = self.failure {
                return Err(io::Error::from_raw_os_error(code));
            }
        }
        self.disk.write(p, t)
    }
    fn sync_file(&mut self, p: &Path) -> io::Result<()> {
        self.disk.sync_file(p)
    }
    fn rename(&mut self, a: &Path, b: &Path) -> io::Result<()> {
        self.disk.rename(a, b)
    }
    fn sync_dir(&mut self, p: &Path) -> io::Result<()> {
        self.disk.sync_dir(p)
    }
    fn remove(&mut self, p: &Path) -> io::Result<()> {
        self.disk.remove(p)
    }
}

#[test]
fn repeated_safe_starts_keep_the_first_quarantine() {
    let mut disk = fs();
    for i in 0..5 {
        disk.seed("/data/oos-ui.setup", &format!("setup {}", i));
        disk = start(disk, true).unwrap().fs.crash();
    }
    assert_eq!(disk.text("/data/oos-ui.setup.bad").as_deref(), Some("setup 0"));
    let quarantines: Vec<_> = disk.files.keys().filter(|p| p.to_string_lossy().contains(".bad")).collect();
    assert_eq!(quarantines.len(), 3);
}

#[test]
fn device_storage_errno_failures_allow_ui_with_no_restore_or_promotion() {
    for code in [libc::ENOENT, libc::EROFS, libc::EIO, libc::ENOSPC] {
        for safe in [false, true] {
            let disk = Shared(std::rc::Rc::new(std::cell::RefCell::new(fs())));
            let mut s = Startup::start(
                DataFailure { disk: disk.clone(), failure: Some(code) },
                Path::new("/data"),
                Path::new("/temporary"),
                "run".into(),
                safe,
            )
            .unwrap();
            assert!(!s.enabled());
            assert!(s.restore().unwrap().is_none());
            let calls = disk.0.borrow().calls;
            s.observe("defaults".into(), Duration::ZERO).unwrap();
            s.observe("defaults".into(), STABLE).unwrap();
            s.clean("defaults".into(), STABLE, false).unwrap();
            assert_eq!(disk.0.borrow().calls, calls);
            assert!(disk.0.borrow().text("/data/oos-ui.running").is_none());
        }
    }
}

#[test]
fn repeated_write_failures_cap_retry_delay_at_five_minutes() {
    let mut s = start(fs(), false).unwrap();
    s.observe("new".into(), Duration::ZERO).unwrap();
    s.fs.cut = Some(s.fs.calls);
    let mut now = STABLE;
    let mut delay = 1;
    for attempt in 0..14 {
        assert_eq!(s.observe("new".into(), now).is_err(), attempt == 0);
        let calls = s.fs.calls;
        now += Duration::from_secs(delay);
        s.observe("new".into(), now - Duration::from_millis(1)).unwrap();
        assert_eq!(s.fs.calls, calls);
        delay = (delay * 2).min(300);
    }
}

#[test]
fn quarantine_keeps_newest_numeric_stamp_across_rtc_digit_boundaries() {
    let mut disk = fs();
    for (name, text) in [("", "first"), (".9", "older"), (".10", "newer")] {
        disk.seed(&format!("/data/oos-ui.setup.bad{}", name), text);
    }
    let state = start(disk, true).unwrap();
    assert_eq!(state.fs.text("/data/oos-ui.setup.bad").as_deref(), Some("first"));
    assert_eq!(state.fs.text("/data/oos-ui.setup.bad.10").as_deref(), Some("newer"));
    assert!(state.fs.text("/data/oos-ui.setup.bad.9").is_none());
}
