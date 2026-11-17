// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn queued_recalls_reject_oversized_text_and_accept_the_exact_limit() {
    let fs = filesystem();
    let memory = fs.memory.clone();
    lock(&memory).seed("/saves/large", &"x".repeat(64 * 1024 + 1));
    lock(&memory).seed("/saves/limit", &"x".repeat(64 * 1024));
    let mut worker = Worker::spawn(fs, None).unwrap();
    for path in ["/saves/large", "/saves/large", "/saves/large", "/saves/limit"] {
        worker.recall(path.into()).unwrap();
    }
    for oversized in [true, true, true, false] {
        let Done::Recall(_, result) = worker.poll(true).unwrap() else { panic!("expected recall") };
        if oversized {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        } else {
            assert_eq!(result.unwrap(), "x".repeat(64 * 1024));
        }
    }
    assert!(!worker.busy());
}

#[test]
fn disk_recall_checks_the_byte_limit_and_utf8() {
    let path = std::env::temp_dir().join(format!("oos-bounded-recall-{}", std::process::id()));
    let disk = crate::store::durable::Disk;
    std::fs::write(&path, "x".repeat(64 * 1024)).unwrap();
    assert_eq!(disk.read_limited(&path, 64 * 1024).unwrap().len(), 64 * 1024);
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(1024 * 1024 * 1024).unwrap();
    assert_eq!(disk.read_limited(&path, 64 * 1024).unwrap_err().kind(), io::ErrorKind::InvalidData);
    std::fs::write(&path, [0xff]).unwrap();
    assert_eq!(disk.read_limited(&path, 64 * 1024).unwrap_err().kind(), io::ErrorKind::InvalidData);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn oversized_observation_restarts_stability_when_the_previous_setup_returns() {
    let fs = filesystem();
    let memory = fs.memory.clone();
    let mut worker = automatic(fs);
    worker.observe("candidate".into(), Duration::ZERO).unwrap();
    assert!(worker.observe("x".repeat(64 * 1024 + 1), Duration::from_secs(30)).is_err());
    for now in [60, 119] {
        worker.observe("candidate".into(), Duration::from_secs(now)).unwrap();
        assert!(!worker.busy(), "returning to the candidate needs another full stability interval");
        assert_eq!(lock(&memory).crash().text("/data/oos-ui.setup").as_deref(), Some("last stable"));
    }
    worker.observe("candidate".into(), Duration::from_secs(120)).unwrap();
    completion(&mut worker).1.unwrap();
    assert_eq!(lock(&memory).crash().text("/data/oos-ui.setup").as_deref(), Some("candidate"));
}

#[test]
fn shutdown_applies_pending_recalls_before_saving_even_after_a_write_error() {
    use crate::{cmd, Source, Ui};
    for failed in [false, true] {
        let mut fs = filesystem();
        let memory = fs.memory.clone();
        let mut recalled = crate::model::Model::default();
        for (slot, scale) in [(1, 12), (2, 14)] {
            recalled.tscale = scale;
            lock(&memory).seed(&format!("/saves/setup{slot}.txt"), &crate::store::setup_text(&recalled));
        }
        let (entered, release) = blocked(&mut fs);
        let mut worker = automatic(fs);
        worker.save("/saves/blocker".into(), "blocked".into()).unwrap();
        entered.recv().unwrap();
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        *get(&mut ui, None).unwrap() = worker;
        ui.store_dir = "/saves".into();
        ui.m.tscale = 16;
        cmd::run(&mut ui, "menu Storage");
        cmd::run(&mut ui, "do Recall setup");
        ui.script_set("Setup slot", "2  (setup2.txt)").unwrap();
        cmd::run(&mut ui, "do Recall setup");
        if failed {
            get(&mut ui, None).unwrap().save("/saves/failed".into(), "error".into()).unwrap();
        }
        assert_eq!(ui.m.tscale, 16);
        release.send(()).unwrap();
        let result = crate::ui::lifecycle::storage::finish(&mut ui, Duration::from_secs(1), true);
        assert_eq!(result.is_err(), failed);
        assert_eq!(ui.m.tscale, 14);
        let fs = lock(&memory).crash();
        for path in ["/data/oos-ui.setup", "/temporary/oos-ui.setup"] {
            assert_eq!(crate::store::parse_setup_lenient(&fs.text(path).unwrap()).0.tscale, 14);
        }
        assert_eq!(fs.text("/data/oos-ui.running"), None);
        assert_eq!(lock(&memory).text("/temporary/oos-ui.off").as_deref(), Some(""));
    }
}
