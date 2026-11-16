// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::calfile;

#[test]
fn audit0073_adc_skips_wrong_size_and_bad_crc_at_each_location() {
    let dir = std::env::temp_dir().join(format!("oos-adc-fallback-{}", std::process::id()));
    let paths = [dir.join("own"), dir.join("factory"), dir.join("factory/default")];
    for p in &paths {
        std::fs::create_dir_all(p).unwrap();
    }
    let dirs: Vec<&str> = paths.iter().map(|p| p.to_str().unwrap()).collect();
    let stamp = (2000, 1, 1, 0, 0, 0);
    for (k, p) in paths.iter().enumerate() {
        std::fs::write(p.join("cal_adc.hex"), calfile::build(&vec![k as u8; 0x738], stamp)).unwrap();
    }
    assert_eq!(CalAdc::load_dirs(&dirs).unwrap().payload(), &[0; 0x738]);
    for (k, path) in paths.iter().enumerate().take(2) {
        std::fs::write(path.join("cal_adc.hex"), calfile::build(&[0; 4], stamp)).unwrap();
        assert_eq!(CalAdc::load_dirs(&dirs).unwrap().payload(), &vec![(k + 1) as u8; 0x738]);
    }
    let mut bad = calfile::build(&vec![3; 0x738], stamp);
    bad[28] ^= 1;
    std::fs::write(paths[0].join("cal_adc.hex"), bad).unwrap();
    assert_eq!(CalAdc::load_dirs(&dirs).unwrap().payload(), &[2; 0x738]);
    std::fs::remove_file(paths[2].join("cal_adc.hex")).unwrap();
    assert!(CalAdc::load_dirs(&dirs).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
