// SPDX-License-Identifier: GPL-2.0-or-later

//! Test-only fake calibration. Parse embedded bytes, never the machine's loader defaults.

use super::{
    adc::CalAdc,
    afe::Cal,
    boot::cal::{CalFiles, DdrCal},
};

pub const ADC: &[u8] = include_bytes!("../../testdata/synthetic/cal_adc.hex");
pub const DDR: &[u8] = include_bytes!("../../testdata/synthetic/cal_ddr.hex");
pub const LSB: &[u8] = include_bytes!("../../testdata/synthetic/cal_lsb.hex");
pub const ZERO: &[u8] = include_bytes!("../../testdata/synthetic/cal_afe_zero.hex");
pub const BW: &[u8] = include_bytes!("../../testdata/synthetic/cal_afe_bandwidth.hex");
pub const LA: &[u8] = include_bytes!("../../testdata/synthetic/cal_la.hex");
pub const AFG: &[u8] = include_bytes!("../../testdata/synthetic/cal_afg.hex");
pub const NAMES: [&str; 8] = [
    "cal_adc.hex",
    "cal_ddr.hex",
    "cal_lsb.hex",
    "cal_afe_zero.hex",
    "cal_afe_bandwidth.hex",
    "cal_vertical.hex",
    "cal_la.hex",
    "cal_afg.hex",
];

pub fn files() -> Vec<(&'static str, Vec<u8>)> {
    [ADC, DDR, LSB, ZERO, BW]
        .into_iter()
        .map(<[u8]>::to_vec)
        .chain([vertical(), LA.to_vec(), AFG.to_vec()])
        .zip(NAMES)
        .map(|(bytes, name)| (name, bytes))
        .collect()
}

/// Explicit private opt-in, without the production loader's fallback directories.
pub fn private(name: &str) -> Vec<u8> {
    let dir = std::env::var("OOS_CAL_DIR").expect("set OOS_CAL_DIR");
    assert!(!dir.trim().is_empty(), "set OOS_CAL_DIR to a nonempty directory");
    std::fs::read(std::path::Path::new(&dir).join(name)).expect(name)
}

/// Same invented formulas as tools/generate-test-cal.py, built only in memory.
pub fn vertical() -> Vec<u8> {
    let mut keys = Vec::<i64>::new();
    for decade in 0..5 {
        let base = 100_000 * 10i64.pow(decade);
        keys.extend((0..50).map(|k| base + k * base / 50));
        keys.extend((0..60).map(|k| 2 * base + k * base / 20));
        keys.extend((0..50).map(|k| 5 * base + k * base / 10));
    }
    keys.push(10_000_000_000);
    let mut payload = Vec::new();
    for group in 0i32..4 {
        for ch in 0i32..4 {
            for (i, &key) in keys.iter().enumerate() {
                let band = (i / 10) as i32;
                let gains = [band % 4, 2, band / 4 % 4, band / 16 % 4, band / 64, ch, group, 0];
                payload.extend(key.to_le_bytes());
                for g in gains {
                    payload.extend(g.to_le_bytes());
                }
                let scale = keys[((band as usize) * 10 + 9).min(800)] as f64 + 0.5;
                payload.extend((scale / key as f64).to_le_bytes());
                payload.extend((32000 + ch * 10 + group).to_le_bytes());
                payload.extend(0i32.to_le_bytes());
            }
        }
    }
    super::calfile::build(&payload, (2000, 1, 1, 0, 0, 0))
}

pub fn adc() -> CalAdc {
    CalAdc::from_file_bytes(ADC).expect("synthetic ADC container")
}

pub fn afe() -> Cal {
    Cal::from_bytes(LSB, ZERO, BW, vertical())
}

pub fn boot() -> CalFiles {
    CalFiles {
        adc: Some(adc()),
        ddr: Some(DdrCal::from_file_bytes(DDR).expect("synthetic DDR container")),
        notes: Vec::new(),
    }
}

#[test]
fn generated_vertical_matches_reference_container() {
    let bytes = vertical();
    assert_eq!(bytes.len(), 717724);
    // CRC of the Python reference generator's complete container.
    assert_eq!(crate::crc32(&bytes), 0x9dce7aed);
}
