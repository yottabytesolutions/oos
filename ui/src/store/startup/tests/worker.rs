// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::store::startup::worker::{Done, Worker};
use crate::ui::lifecycle::storage::get;
use oos_hal::sync::lock;
use std::sync::{mpsc, Arc, Mutex};

mod review_fixes;

type Gate = (mpsc::SyncSender<()>, mpsc::Receiver<()>);
#[derive(Debug, PartialEq)]
enum Event {
    Write(PathBuf, String),
    Remove(PathBuf),
}
struct Filesystem {
    memory: Arc<Mutex<Memory>>,
    gate: Option<Gate>,
    events: Arc<Mutex<Vec<Event>>>,
}
impl Files for Filesystem {
    fn prepare(&mut self, _: &Path) -> io::Result<()> {
        Ok(())
    }
    fn entries(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        lock(&self.memory).entries(path)
    }
    fn read(&self, path: &Path) -> io::Result<String> {
        lock(&self.memory).read(path)
    }
    fn read_limited(&self, path: &Path, limit: usize) -> io::Result<String> {
        lock(&self.memory).read_limited(path, limit)
    }
    fn write(&mut self, path: &Path, text: &str) -> io::Result<()> {
        if text == "blocked" {
            if let Some((entered, release)) = self.gate.take() {
                entered.send(()).unwrap();
                release.recv().unwrap();
            }
        }
        if text == "error" {
            return Err(io::Error::other("injected write failure"));
        }
        lock(&self.memory).write(path, text)?;
        lock(&self.events).push(Event::Write(path.into(), text.into()));
        Ok(())
    }
    fn sync_file(&mut self, path: &Path) -> io::Result<()> {
        lock(&self.memory).sync_file(path)
    }
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        lock(&self.memory).rename(from, to)
    }
    fn sync_dir(&mut self, path: &Path) -> io::Result<()> {
        lock(&self.memory).sync_dir(path)
    }
    fn remove(&mut self, path: &Path) -> io::Result<()> {
        lock(&self.memory).remove(path)?;
        lock(&self.events).push(Event::Remove(path.into()));
        Ok(())
    }
}

fn filesystem() -> Filesystem {
    Filesystem { memory: Arc::new(Mutex::new(fs())), gate: None, events: Default::default() }
}

fn automatic(fs: Filesystem) -> Worker {
    let startup = Startup::start(fs, Path::new("/data"), Path::new("/temporary"), "run".into(), false).unwrap();
    Worker::spawn(filesystem(), Some(startup)).unwrap()
}

fn blocked(fs: &mut Filesystem) -> (mpsc::Receiver<()>, mpsc::SyncSender<()>) {
    let (started, entered) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    fs.gate = Some((started, gate));
    (entered, release)
}

fn completion(worker: &mut Worker) -> (Option<Box<Path>>, io::Result<()>) {
    worker.wait_ready();
    match worker.poll(false).unwrap() {
        Done::Save(path, result) => (Some(path), result),
        Done::Automatic(result) => (None, result),
        Done::Recall(_, _) => panic!("expected a save completion"),
    }
}

#[test]
fn blocked_filesystem_leaves_manual_save_and_frame_steps_responsive() {
    use crate::{Source, Ui};
    let mut fs = filesystem();
    let memory = fs.memory.clone();
    let (entered, release) = blocked(&mut fs);
    let mut worker = Worker::spawn(fs, None).unwrap();
    worker.save("/saves/first".into(), "blocked".into()).unwrap();
    entered.recv().unwrap();
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    *get(&mut ui, None).unwrap() = worker;
    ui.store_dir = "/saves".into();
    ui.m.quick = crate::model::Quick::SaveSetup;
    let expected = crate::store::setup_text(&ui.m);
    ui.key(crate::ui::key::QUICK);
    ui.m.tscale += 1;
    ui.step(0.033);
    assert!(get(&mut ui, None).unwrap().poll(false).is_none());
    release.send(()).unwrap();
    get(&mut ui, None).unwrap().flush().unwrap();
    assert_eq!(lock(&memory).crash().text("/saves/setup1.txt").as_deref(), Some(expected.as_str()));
}

#[test]
fn completions_and_errors_preserve_explicit_save_order() {
    let fs = filesystem();
    let memory = fs.memory.clone();
    let mut worker = Worker::spawn(fs, None).unwrap();
    for text in ["first", "error", "last"] {
        worker.save("/saves/same".into(), text.into()).unwrap();
    }
    for success in [true, false, true] {
        let (path, result) = completion(&mut worker);
        assert_eq!(path.as_deref(), Some(Path::new("/saves/same")));
        assert_eq!(result.is_ok(), success);
    }
    assert_eq!(lock(&memory).crash().text("/saves/same").as_deref(), Some("last"));
}

#[test]
fn queue_and_payload_limits_include_unconsumed_completions() {
    let mut fs = filesystem();
    let (entered, release) = blocked(&mut fs);
    let mut worker = Worker::spawn(fs, None).unwrap();
    worker.save("/saves/first".into(), "blocked".into()).unwrap();
    entered.recv().unwrap();
    for _ in 1..4 {
        worker.save("/saves/next".into(), "x".repeat(64 * 1024)).unwrap();
    }
    assert_eq!(worker.save("/saves/overflow".into(), "extra".into()).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    assert_eq!(
        worker.save("/saves/large".into(), "x".repeat(64 * 1024 + 1)).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(worker.save("x".repeat(4097).into(), "x".into()).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert_eq!(worker.observe("x".repeat(64 * 1024 + 1), STABLE).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    release.send(()).unwrap();
    worker.wait_ready();
    assert_eq!(worker.save("/saves/still-full".into(), "extra".into()).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    completion(&mut worker).1.unwrap();
    worker.save("/saves/admitted".into(), "last".into()).unwrap();
    worker.flush().unwrap();
}

#[test]
fn edits_during_pending_promotion_coalesce_and_transients_restart_stability() {
    let mut fs = filesystem();
    let memory = fs.memory.clone();
    let (entered, release) = blocked(&mut fs);
    let mut worker = automatic(fs);
    worker.observe("blocked".into(), Duration::ZERO).unwrap();
    worker.observe("blocked".into(), STABLE).unwrap();
    entered.recv().unwrap();
    for (text, seconds) in [("latest", 61), ("transient", 62), ("latest", 63)] {
        worker.observe(text.into(), Duration::from_secs(seconds)).unwrap();
    }
    release.send(()).unwrap();
    completion(&mut worker).1.unwrap();
    worker.observe("latest".into(), Duration::from_secs(122)).unwrap();
    assert!(worker.poll(false).is_none(), "returning to the same text still starts a new interval");
    assert_eq!(lock(&memory).crash().text("/data/oos-ui.setup").as_deref(), Some("blocked"));
    worker.observe("latest".into(), Duration::from_secs(123)).unwrap();
    completion(&mut worker).1.unwrap();
    assert_eq!(lock(&memory).crash().text("/data/oos-ui.setup").as_deref(), Some("latest"));
}

#[test]
fn shutdown_flushes_accepted_saves_before_current_setup_and_marker_cleanup() {
    let mut fs = filesystem();
    let memory = fs.memory.clone();
    let events = fs.events.clone();
    let (entered, release) = blocked(&mut fs);
    let mut worker = automatic(fs);
    lock(&events).clear();
    worker.save("/data/oos-ui.setup".into(), "blocked".into()).unwrap();
    entered.recv().unwrap();
    worker.save("/data/oos-ui.setup".into(), "second".into()).unwrap();
    let shutdown = std::thread::spawn(move || worker.finish("shutdown".into(), Duration::from_secs(2), true));
    release.send(()).unwrap();
    shutdown.join().unwrap().unwrap();
    let fs = lock(&memory).crash();
    assert_eq!(fs.text("/data/oos-ui.setup").as_deref(), Some("shutdown"));
    assert_eq!(fs.text("/temporary/oos-ui.setup").as_deref(), Some("shutdown"));
    assert_eq!(fs.text("/data/oos-ui.running"), None);
    assert_eq!(fs.text("/data/oos-ui.stable"), None);
    assert_eq!(lock(&memory).text("/temporary/oos-ui.off").as_deref(), Some(""));
    assert_eq!(
        *lock(&events),
        [
            Event::Write("/data/oos-ui.setup.new".into(), "blocked".into()),
            Event::Write("/data/oos-ui.setup.new".into(), "second".into()),
            Event::Write("/temporary/oos-ui.setup.new".into(), "shutdown".into()),
            Event::Write("/data/oos-ui.setup.new".into(), "shutdown".into()),
            Event::Remove("/data/oos-ui.running".into()),
            Event::Remove("/data/oos-ui.stable".into()),
            Event::Write("/temporary/oos-ui.off".into(), "".into()),
        ]
    );
}

#[test]
fn ordinary_shutdown_only_promotes_a_stable_candidate() {
    for (now, expected) in [(59, "last stable"), (60, "candidate")] {
        let fs = filesystem();
        let memory = fs.memory.clone();
        let mut worker = automatic(fs);
        worker.observe("candidate".into(), Duration::ZERO).unwrap();
        worker.finish("candidate".into(), Duration::from_secs(now), false).unwrap();
        assert_eq!(lock(&memory).crash().text("/data/oos-ui.setup").as_deref(), Some(expected));
    }
}

#[test]
fn dropping_worker_flushes_manual_saves_for_scripts_and_host_shutdown() {
    let fs = filesystem();
    let memory = fs.memory.clone();
    let mut worker = Worker::spawn(fs, None).unwrap();
    worker.save("/saves/setup".into(), "first".into()).unwrap();
    worker.save("/saves/setup".into(), "last".into()).unwrap();
    drop(worker);
    assert_eq!(lock(&memory).crash().text("/saves/setup").as_deref(), Some("last"));
}

#[test]
fn failed_save_does_not_skip_later_writes_or_shutdown_cleanup() {
    let fs = filesystem();
    let memory = fs.memory.clone();
    let mut worker = automatic(fs);
    worker.save("/saves/failed".into(), "error".into()).unwrap();
    worker.save("/saves/next".into(), "accepted".into()).unwrap();
    assert!(worker.finish("shutdown".into(), Duration::ZERO, true).is_err());
    let fs = lock(&memory).crash();
    assert_eq!(fs.text("/saves/next").as_deref(), Some("accepted"));
    assert_eq!(fs.text("/data/oos-ui.setup").as_deref(), Some("shutdown"));
    assert_eq!(fs.text("/data/oos-ui.running"), None);
}

#[test]
fn automatic_errors_and_retries_use_injected_observation_time() {
    let fs = filesystem();
    let memory = fs.memory.clone();
    let mut worker = automatic(fs);
    {
        let mut fs = lock(&memory);
        fs.cut = Some(fs.calls);
    }
    worker.observe("new".into(), Duration::ZERO).unwrap();
    worker.observe("new".into(), STABLE).unwrap();
    let (path, result) = completion(&mut worker);
    assert!(path.is_none() && result.is_err());
    lock(&memory).cut = None;
    worker.observe("new".into(), STABLE + Duration::from_millis(999)).unwrap();
    completion(&mut worker).1.unwrap();
    assert_eq!(lock(&memory).crash().text("/data/oos-ui.setup").as_deref(), Some("last stable"));
    worker.observe("new".into(), STABLE + Duration::from_secs(1)).unwrap();
    completion(&mut worker).1.unwrap();
    assert_eq!(lock(&memory).crash().text("/data/oos-ui.setup").as_deref(), Some("new"));
}

#[test]
fn fifo_save_then_recall_waits_without_blocking_frames_and_reads_before_later_saves() {
    use crate::{cmd, Source, Ui};
    for previous in [None, Some("tscale=8\n")] {
        let mut fs = filesystem();
        let memory = fs.memory.clone();
        if let Some(text) = previous {
            lock(&memory).seed("/saves/setup1.txt", text);
        }
        let (entered, release) = blocked(&mut fs);
        let mut worker = Worker::spawn(fs, None).unwrap();
        worker.save("/saves/blocker".into(), "blocked".into()).unwrap();
        entered.recv().unwrap();
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        *get(&mut ui, None).unwrap() = worker;
        ui.store_dir = "/saves".into();
        ui.m.tscale = 12;
        // Device intake dispatches each FIFO line through cmd::run without an intervening frame or save barrier.
        for line in "menu Storage\ndo Save setup\ndo Recall setup\n".lines() {
            cmd::run(&mut ui, line);
        }
        ui.m.tscale = 16;
        cmd::run(&mut ui, "do Save setup");
        ui.store_dir = "/saves/elsewhere".into();
        ui.step(0.033);
        assert_eq!(ui.m.tscale, 16, "the queued recall must not block intake or a frame");
        release.send(()).unwrap();
        while get(&mut ui, None).unwrap().busy() {
            get(&mut ui, None).unwrap().wait_ready();
            ui.step(0.033);
        }
        assert_eq!(ui.m.tscale, 12, "recall must read the preceding save, even before a later replacement");
        let text = lock(&memory).crash().text("/saves/setup1.txt").unwrap();
        assert_eq!(crate::store::parse_setup_lenient(&text).0.tscale, 16);
        assert_eq!(ui.store_dir, Path::new("/saves/elsewhere"), "recall uses its captured path");
    }
}

#[test]
fn successful_promotions_stay_coalesced_until_the_candidate_changes() {
    let mut worker = automatic(filesystem());
    worker.observe("first".into(), Duration::ZERO).unwrap();
    worker.observe("first".into(), STABLE).unwrap();
    completion(&mut worker).1.unwrap();
    for now in [STABLE, STABLE * 2, STABLE * 100] {
        worker.observe("first".into(), now).unwrap();
        assert!(worker.poll(false).is_none());
        assert!(!worker.busy(), "an unchanged promoted candidate must not submit any job");
    }
    worker.observe("second".into(), STABLE * 100).unwrap();
    worker.observe("second".into(), STABLE * 101).unwrap();
    assert!(worker.busy());
    completion(&mut worker).1.unwrap();
    worker.observe("second".into(), STABLE * 102).unwrap();
    assert!(!worker.busy());
}

#[test]
fn oversized_shutdown_reports_the_error_and_still_cleans_up_and_stops_the_supervisor() {
    for power in [false, true] {
        let fs = filesystem();
        let memory = fs.memory.clone();
        let mut worker = automatic(fs);
        worker.save("/saves/accepted".into(), "saved".into()).unwrap();
        let error = worker.finish("x".repeat(64 * 1024 + 1), Duration::ZERO, power).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let fs = lock(&memory);
        assert_eq!(fs.text("/saves/accepted").as_deref(), Some("saved"));
        assert_eq!(fs.text("/data/oos-ui.setup").as_deref(), Some("last stable"));
        assert_eq!(fs.text("/temporary/oos-ui.setup").as_deref(), Some("last stable"));
        assert_eq!(fs.text("/data/oos-ui.running"), None);
        assert_eq!(fs.text("/data/oos-ui.stable"), None);
        assert_eq!(fs.text("/temporary/oos-ui.off").is_some(), power);
    }
}
