// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::{fs, path::PathBuf};

const CARD_A: Key = [1; 16];
const CARD_B: Key = [2; 16];

fn path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("oos-card-{name}-{}.state", std::process::id()))
}

fn detect(mode: Mode, ceiling: Mode) -> Status {
    Status::from_probe(Ok(mode), Ok(ceiling))
}

fn observe(path: &Path, key: Key, mode: Mode) -> Status {
    let (status, saved) = record_with(Some(path), Some(key), detect(mode, Mode::Sdr104), Some("test-build"), false);
    saved.unwrap();
    status
}

#[test]
fn stores_a_sha256_prefix_of_the_cid_bytes() {
    let key = card_key("00112233445566778899AABBCCDDEEFF\n").unwrap();
    assert_eq!(hex(&key), "a8faed6abbf35c12a4b26e40f6feb19d");
    for bad in ["", "00112233", "00112233445566778899ZZbbccddeeff", "0011223344556677ﬀ"] {
        assert!(card_key(bad).is_none());
    }
}

#[test]
fn round_trip_keeps_only_the_latest_sixteen_starts() {
    let mut record = History::new(CARD_A);
    for n in 0..20 {
        let mut status = detect(if n == 0 { Mode::Sdr104 } else { Mode::Hs }, Mode::Sdr104);
        status.kexec = n % 2 == 0;
        record.append(status, Some(&format!("build {n}")));
    }
    let encoded = record.encode();
    assert_eq!(History::decode(&encoded), Some(record));
    assert!(encoded.starts_with("oos-sd-card 1\nkey=01010101010101010101010101010101\nbest=sdr104\n"));
    assert!(encoded.contains("starts=20\nkexec=10\ndegraded=19\nworse=0\ndegraded_in_a_row=10\n"));
    assert_eq!(encoded.matches("start=").count(), HISTORY_LEN);
    assert!(encoded.contains("start=kexec\ths\tbuild 4\n"));
    assert!(encoded.ends_with("start=ordinary\ths\tbuild 19\nend\n"));
    assert!(!encoded.contains("\tbuild 3\n"));
}

#[test]
fn missing_truncated_old_and_oversized_history_start_fresh() {
    let path = path("tolerance");
    let good = || observe(&path, CARD_A, Mode::Sdr104);
    let read = || record::read_bounded(&path, MAX_BYTES).unwrap();
    assert_eq!(good().state(), CardState::Ok);
    let valid = read();
    for broken in [
        valid.replace("oos-sd-card 1", "oos-sd-card 0"),
        valid.trim_end_matches("end\n").into(),
        valid.replace("start=ordinary", "start="),
        valid.replace("starts=1", "starts=2"),
        "x".repeat(MAX_BYTES as usize + 1),
    ] {
        fs::write(&path, broken).unwrap();
        let status = observe(&path, CARD_A, Mode::Hs);
        assert_eq!(
            (status.expected(), status.state(), status.degraded_in_a_row()),
            (Mode::Hs, CardState::Unverified, 0)
        );
        assert_eq!(History::decode(&read()).unwrap().counts.starts, 1);
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn learns_the_ceiling_and_resets_the_streak_and_history_on_card_swap() {
    let path = path("learning");
    let start = |mode| observe(&path, CARD_A, mode);
    let low = start(Mode::Legacy);
    assert_eq!((low.expected(), low.ceiling(), low.state()), (Mode::Legacy, Mode::Sdr104, CardState::Unverified));
    assert_eq!(start(Mode::Hs).state(), CardState::Unverified);
    assert_eq!(start(Mode::Sdr104).state(), CardState::Ok);
    assert_eq!(start(Mode::Hs).degraded_in_a_row(), 1);
    let worse = start(Mode::Legacy);
    assert_eq!((worse.expected(), worse.state(), worse.degraded_in_a_row()), (Mode::Sdr104, CardState::Worse, 2));
    assert_eq!(start(Mode::Unknown).degraded_in_a_row(), 0);
    let (kexec, saved) = record_with(Some(&path), Some(CARD_A), detect(Mode::Hs, Mode::Sdr104), None, true);
    saved.unwrap();
    assert!(kexec.is_kexec());
    assert_eq!((kexec.state(), kexec.degraded_in_a_row()), (CardState::Degraded, 0));
    let (lower, saved) = record_with(Some(&path), Some(CARD_A), detect(Mode::Hs, Mode::Hs), None, false);
    saved.unwrap();
    assert_eq!((lower.expected(), lower.state(), lower.degraded_in_a_row()), (Mode::Hs, CardState::Ok, 0));
    assert_eq!(observe(&path, CARD_B, Mode::Hs).state(), CardState::Unverified);
    let replaced = History::decode(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!((replaced.key, replaced.best, replaced.counts.starts), (CARD_B, Mode::Hs, 1));
    fs::remove_file(path).unwrap();
}

#[test]
fn missing_identity_and_write_errors_leave_a_usable_status() {
    let path = path("no-write").join("state");
    let detection = detect(Mode::Hs, Mode::Sdr104);
    let (status, saved) = record_with(Some(&path), None, detection, None, false);
    assert!(saved.is_ok());
    assert_eq!(status.state(), CardState::Unverified);
    let (status, saved) = record_with(Some(&path), Some(CARD_A), detection, None, false);
    assert_eq!(saved.unwrap_err().kind(), io::ErrorKind::NotFound);
    assert_eq!(status.state(), CardState::Unverified);
}

#[test]
fn a_partial_temporary_file_from_a_power_cut_does_not_prevent_the_next_write() {
    let path = path("interrupted-write");
    assert_eq!(observe(&path, CARD_A, Mode::Sdr104).state(), CardState::Ok);
    let temporary = path.with_extension("new");
    fs::write(&temporary, "oos-sd-card 1\nkey=").unwrap();
    let next = observe(&path, CARD_A, Mode::Hs);
    assert_eq!((next.state(), next.degraded_in_a_row()), (CardState::Degraded, 1));
    assert!(!temporary.exists());
    fs::remove_file(path).unwrap();
}

#[test]
fn kexec_neither_increments_nor_resets_the_slow_streak() {
    let mut history = History::new(CARD_A);
    let mut start = |mode, kexec| {
        let mut status = detect(mode, Mode::Sdr104);
        status.kexec = kexec;
        history.append(status, Some("test-build")).degraded_in_a_row()
    };
    assert_eq!(start(Mode::Sdr104, false), 0);
    assert_eq!(start(Mode::Hs, false), 1);
    assert_eq!(start(Mode::Legacy, true), 1);
    assert_eq!(start(Mode::Sdr104, true), 1);
    assert_eq!(start(Mode::Sdr12, false), 2);
    assert_eq!(start(Mode::Unknown, false), 0);
    let saved = history.encode();
    assert!(saved.contains("start=kexec\tlegacy\ttest-build\nstart=kexec\tsdr104\ttest-build\n"));
}

#[test]
fn closed_data_refuses_card_history() {
    use crate::system::admission_test::{assert_refused, with_closed_data};
    with_closed_data(module_path!(), "closed_data_refuses_card_history", |data, ram| {
        let path = data.join("sd-card.state");
        let (status, result) = record_with(Some(&path), Some(CARD_A), detect(Mode::Hs, Mode::Hs), None, false);
        assert_refused(result.unwrap_err());
        assert_eq!(status.mode, Mode::Hs);
        assert!(!path.exists());
        record::write_state(&ram.join("sd-card.state"), status).unwrap();
    });
}
