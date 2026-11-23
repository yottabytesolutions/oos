// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::{
    fs::FileTimes,
    os::unix::fs::symlink,
    time::{Duration, SystemTime},
};

#[test]
fn validates_selected_group_names_and_rejects_links() {
    let scratch = Scratch::for_test();
    let root = scratch.path();
    fs::create_dir(root.join("dense")).unwrap();
    fs::create_dir(root.join("short")).unwrap();
    assert_eq!(groups(root, &[]).unwrap(), ["dense", "short"]);
    assert_eq!(groups(root, &["short"]).unwrap(), ["short"]);
    for invalid in ["", ".", "..", "../dense", "dense/", "dense/short", "dense\n"] {
        assert!(groups(root, &[invalid]).unwrap_err().contains("without control characters"));
    }
    assert_eq!(groups(root, &["dense", "dense"]).unwrap_err(), "fixture groups must be unique");
    symlink(root.join("dense"), root.join("link")).unwrap();
    assert_eq!(groups(root, &["link"]).unwrap_err(), "each selected fixture group must be a real directory");
}

#[test]
fn fixture_identity_tracks_relative_names_and_contents_only() {
    let scratch = Scratch::for_test();
    let first = scratch.path().join("first");
    let second = scratch.path().join("second");
    for root in [&first, &second] {
        fs::create_dir_all(root.join("nested")).unwrap();
    }
    fs::write(first.join("nested/data.bin"), b"payload").unwrap();
    fs::write(first.join("fixture.txt"), b"metadata").unwrap();
    fs::write(second.join("fixture.txt"), b"metadata").unwrap();
    let data = second.join("nested/data.bin");
    fs::write(&data, b"payload").unwrap();
    File::open(&data)
        .unwrap()
        .set_times(FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000)))
        .unwrap();

    let expected = content::identity(&first, &scratch).unwrap();
    assert_eq!(content::identity(&second, &scratch).unwrap(), expected);
    fs::write(&data, b"PAYLOAD").unwrap();
    assert_ne!(content::identity(&second, &scratch).unwrap(), expected);
    fs::write(&data, b"payload").unwrap();
    fs::rename(&data, second.join("nested/renamed.bin")).unwrap();
    assert_ne!(content::identity(&second, &scratch).unwrap(), expected);
}

#[derive(Default)]
struct CacheHit(Vec<String>);

impl Remote for CacheHit {
    fn run(&mut self, operation: &str, script: &str, seconds: u64, input: Option<File>) -> Result<String, String> {
        assert_eq!(seconds, 15);
        assert!(input.is_none());
        self.0.push(script.to_owned());
        match operation {
            "check benchmark cache" => Ok(script.split_once("bin-").unwrap().1[..64].to_owned() + "\n"),
            "check fixture cache" => Ok("present\n".into()),
            _ => panic!("unexpected cache operation: {operation}"),
        }
    }
}

#[test]
fn cache_hits_reuse_the_binary_and_content_addressed_fixture_group() {
    let scratch = Scratch::for_test();
    let benchmark = scratch.path().join("benchmark");
    let fixtures = scratch.path().join("fixtures");
    fs::write(&benchmark, b"benchmark").unwrap();
    fs::create_dir(&fixtures).unwrap();
    fs::write(fixtures.join("fixture.txt"), b"fixture").unwrap();
    let mut remote = CacheHit::default();
    let (runner, digest) = binary(&mut remote, &scratch, &benchmark).unwrap();
    let group = fixture(&mut remote, &scratch, &fixtures).unwrap();
    assert_eq!(runner, format!("{CACHE}/bin-{digest}"));
    assert_eq!(group.path, format!("{CACHE}/group-{}", group.sha256));
    assert_eq!(remote.0.len(), 2);
    assert!(remote.0[0].contains(&format!("sha256sum bin-{digest}")));
    assert!(remote.0[1].contains(&format!("[ -d group-{} ]", group.sha256)));
}
