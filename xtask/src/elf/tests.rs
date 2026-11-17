// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

pub(crate) fn executable() -> Vec<u8> {
    let mut data = vec![0; 120];
    data[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    for (offset, size, value) in [
        (16, 2, 2_u64),
        (18, 2, 183),
        (24, 8, 0x10000),
        (32, 8, 64),
        (54, 2, 56),
        (56, 2, 1),
        (64, 4, 1),
        (68, 4, 5),
        (80, 8, 0x10000),
        (96, 8, 120),
        (104, 8, 120),
    ] {
        data[offset..offset + size].copy_from_slice(&value.to_le_bytes()[..size]);
    }
    data
}

#[test]
fn accepts_static_aarch64_with_entry_in_executable_content() {
    assert_eq!(validate(&executable()), Ok(()));
    let mut padded = executable();
    padded.resize(128, 0);
    padded[54] = 64;
    assert_eq!(validate(&padded), Ok(()), "program headers may include padding");
}

#[test]
fn rejects_invalid_identity_and_truncated_headers_without_panicking() {
    let valid = executable();
    for length in 0..valid.len() {
        assert!(validate(&valid[..length]).is_err(), "length {length}");
    }
    for (offset, value) in [(0, 0), (4, 1), (5, 2), (6, 0), (16, 3), (18, 62), (54, 1), (56, 0), (56, 2)] {
        let mut broken = valid.clone();
        broken[offset] = value;
        assert!(validate(&broken).is_err(), "offset {offset}");
    }
    let mut broken = valid;
    broken[32..40].fill(0xff);
    assert!(validate(&broken).is_err());
}

#[test]
fn rejects_dynamic_and_missing_load_segments_even_after_a_valid_load() {
    for kind in [0, 2, 3] {
        let mut broken = executable();
        broken[64] = kind;
        assert!(validate(&broken).is_err());
    }
    for kind in [2, 3] {
        let mut broken = executable();
        broken.resize(176, 0);
        broken[56] = 2;
        broken[120] = kind;
        assert!(validate(&broken).is_err());
    }
}

#[test]
fn rejects_truncated_load_and_entry_in_bss_or_non_executable_content() {
    for (offset, value) in [(68, 4), (72, 1), (80, 1), (96, 0), (96, 121), (104, 119), (24, 255)] {
        let mut broken = executable();
        broken[offset] = value;
        assert!(validate(&broken).is_err(), "offset {offset}");
    }
    let mut bss = executable();
    bss[24..32].copy_from_slice(&(0x10000_u64 + 120).to_le_bytes());
    bss[104] = 121;
    assert!(validate(&bss).is_err());
    let mut overflow = executable();
    overflow[72..80].fill(0xff);
    assert!(validate(&overflow).is_err());
}

#[test]
fn command_checks_multiple_files_and_reports_unreadable_inputs() {
    let root = crate::testrepo::repo("elf-files", &[]);
    let a = root.join("a");
    let b = root.join("b");
    std::fs::write(&a, executable()).unwrap();
    std::fs::write(&b, executable()).unwrap();
    assert!(run(&[a.to_str().unwrap(), b.to_str().unwrap()]));
    std::fs::write(&b, b"broken").unwrap();
    assert!(!run(&[a.to_str().unwrap(), b.to_str().unwrap()]));
    assert!(file(&root.join("missing")).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
