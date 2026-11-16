// SPDX-License-Identifier: GPL-2.0-or-later

use super::{rec_bytes, Rec, Results};
use crate::dho::calfile::{self, SaveError};

/// Writes the results to OOS's own files (the current OOS file, else the factory file, as the base for the rows and
/// channels not calibrated). Returns the files written.
pub fn save(r: &Results, rigol_dir: &str) -> Result<Vec<String>, SaveError> {
    save_to(r, rigol_dir, crate::dho::calfile::OOS_CAL_DIR)
}

fn save_to(r: &Results, rigol_dir: &str, own_dir: &str) -> Result<Vec<String>, SaveError> {
    let base = |n: &str| -> Result<Vec<u8>, String> {
        let (b, _) = calfile::load_first(n, &[own_dir, rigol_dir]).ok_or(format!("no valid {}", n))?;
        Ok(calfile::parse(&b).map_err(|e| format!("{}: {:?}", n, e))?.to_vec())
    };
    let mut files = Vec::new();
    let stamp = calfile::now();
    if !r.blocks.is_empty() {
        let p = vertical_payload(&base("cal_vertical.hex").map_err(std::io::Error::other)?, &r.blocks)
            .map_err(std::io::Error::other)?;
        files.push(("cal_vertical.hex", p));
    }
    if let Some(adc) = &r.adc {
        files.push(("cal_adc.hex", adc.payload().to_vec()));
    }
    if !r.zero.is_empty() {
        let mut p = base("cal_afe_zero.hex").map_err(std::io::Error::other)?;
        for (ch, z) in &r.zero {
            p = zero_payload(&p, *ch, z);
        }
        files.push(("cal_afe_zero.hex", p));
    }
    let refs: Vec<_> = files.iter().map(|(name, payload)| (*name, payload.as_slice())).collect();
    calfile::save_all(own_dir, &refs, stamp)?;
    Ok(files.iter().map(|(name, _)| format!("{}/{}", own_dir, name)).collect())
}

/// cal_vertical payload with some blocks replaced: `base` = the current payload (12 or 16 blocks x 801 x 56 bytes).
/// Extend missing groups from the last group, matching `Cal::rec` for untouched channels.
pub fn vertical_payload(base: &[u8], blocks: &[(usize, usize, Vec<Rec>)]) -> Result<Vec<u8>, String> {
    let group = 4 * 801 * 56;
    if base.is_empty() || !base.len().is_multiple_of(group) {
        return Err("cal_vertical.hex: payload must contain whole nonempty groups".into());
    }
    let mut p = base.to_vec();
    for (var, ch, recs) in blocks {
        while p.len() < (var + 1) * group {
            p.extend_from_within(p.len() - group..);
        }
        for (i, r) in recs.iter().enumerate().take(801) {
            let o = ((var * 4 + ch) * 801 + i) * 56;
            p[o..o + 56].copy_from_slice(&rec_bytes(r));
        }
    }
    Ok(p)
}

/// cal_afe_zero payload (4 x 20 s32) with channel `ch` replaced.
pub fn zero_payload(base: &[u8], ch: usize, z: &[i32; 20]) -> Vec<u8> {
    let mut p = base.to_vec();
    for (k, v) in z.iter().enumerate() {
        let o = (ch * 20 + k) * 4;
        p[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dho::{afe::Cal, calfile, fixtures};

    #[test]
    fn audit0073_vertical_rejects_partial_or_empty_groups() {
        let group = 4 * 801 * 56;
        for len in [0, 1, group - 1, group + 1, 3 * group + 56] {
            let base = vec![0; len];
            assert!(vertical_payload(&base, &[]).is_err(), "accepted {len} bytes");
            assert!(vertical_payload(&base, &[(3, 0, vec![])]).is_err());
        }
    }

    #[test]
    fn audit0073_vertical_extends_legacy_groups_preserving_fallback() {
        let bytes = fixtures::vertical();
        let base = &calfile::parse(&bytes).unwrap()[..12 * 801 * 56];
        let cal = fixtures::afe();
        let mut block = cal.block(2, 1);
        block[0].zero += 17;
        let got = vertical_payload(base, &[(3, 1, block.clone())]).unwrap();
        assert_eq!(got.len(), 16 * 801 * 56);
        assert_eq!(&got[..base.len()], base);
        let group = 4 * 801 * 56;
        for ch in [0, 2, 3] {
            let o = ch * 801 * 56;
            assert_eq!(&got[3 * group + o..3 * group + o + 801 * 56], &base[2 * group + o..2 * group + o + 801 * 56]);
        }
        let b = calfile::build(&got, (2000, 1, 1, 0, 0, 0));
        let loaded = Cal::from_bytes(fixtures::LSB, fixtures::ZERO, fixtures::BW, b);
        assert_eq!(loaded.rec(3, 1, 0).zero, block[0].zero);
        assert_eq!(vertical_payload(base, &[(2, 1, cal.block(2, 1))]).unwrap().len(), base.len());
    }

    #[test]
    fn audit0073_save_stages_every_file_before_replacing_any() {
        let dir = std::env::temp_dir().join(format!("oos-selfcal-stage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let own = dir.to_str().unwrap();
        let mut r = Results::default();
        r.blocks.push((0, 0, fixtures::afe().block(0, 0)));
        r.adc = Some(crate::dho::adc::CalAdc::from_payload(vec![9; 0x738]).unwrap());
        r.zero.push((0, [7; 20]));
        for name in ["cal_vertical.hex", "cal_adc.hex", "cal_afe_zero.hex"] {
            std::fs::write(dir.join(name), fixtures::files().into_iter().find(|(n, _)| *n == name).unwrap().1).unwrap();
        }
        std::fs::create_dir(dir.join("cal_afe_zero.hex.new")).unwrap();
        assert!(save_to(&r, own, own).is_err());
        for name in ["cal_vertical.hex", "cal_adc.hex", "cal_afe_zero.hex"] {
            assert!(
                std::fs::read(dir.join(name)).unwrap()
                    == fixtures::files().into_iter().find(|(n, _)| *n == name).unwrap().1,
                "{name} changed on failure"
            );
        }
        std::fs::remove_dir(dir.join("cal_afe_zero.hex.new")).unwrap();
        assert_eq!(save_to(&r, own, own).unwrap().len(), 3);
        for name in ["cal_vertical.hex", "cal_adc.hex", "cal_afe_zero.hex"] {
            assert_eq!(
                std::fs::read(dir.join(format!("{name}.bak"))).unwrap(),
                fixtures::files().into_iter().find(|(n, _)| *n == name).unwrap().1
            );
            assert!(calfile::parse(&std::fs::read(dir.join(name)).unwrap()).is_ok());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
