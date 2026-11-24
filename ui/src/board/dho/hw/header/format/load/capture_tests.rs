// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn reduced_capture_round_trips_with_its_raw_geometry() {
    let cores = Cores::default();
    for (peak, inferred, capture) in [(false, false, "decimate:4"), (true, true, "peak:4")] {
        let mut settings = seq::Settings { tdiv: 1e-6, depth: 14_064, window: None, ..Default::default() };
        let params = settings.reduce_capture(peak, 4, inferred).unwrap();
        let input = super::super::Input::new((&settings, &params), (3, 3, false), (&cores, false));
        let meta = input.encode("host-test");
        assert_eq!(input.aligned_len(), Some(3 * 2 * params.memsel as usize));
        assert!(meta.contains(&format!("capture={capture}\ncapture_inferred={inferred}\n")));
        let (fields, _) = parse(&meta).unwrap();
        let (restored, geometry_from_dump) = geometry(&fields).unwrap();
        assert_eq!(restored.mode, settings.mode);
        assert_eq!(geometry_from_dump, params);
        let invalid = meta.replace(&format!("capture={capture}"), "capture=peak:3");
        let (fields, _) = parse(&invalid).unwrap();
        assert!(geometry(&fields).is_err());
    }
}
