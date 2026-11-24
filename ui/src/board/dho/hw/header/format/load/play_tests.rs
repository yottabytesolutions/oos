// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn stock_play_dump_reconstructs_and_validates_the_dma_geometry() {
    let s = seq::Settings {
        tdiv: 1e-6,
        mask: 3,
        window: None,
        header: true,
        playback: Playback::StockInferred,
        ..Default::default()
    };
    let p = hz::prep(3, seq::tdiv_fs(s.tdiv), 7032, 0);
    let cores = Cores::default();
    let input = super::super::Input::new((&s, &p), (3, 3, false), (&cores, false));
    let meta = input.encode("host-test");
    assert_eq!(input.aligned_len(), Some(3 * 28_016));
    assert!(meta.contains("play=stock\nplay_inferred=true\nplay_window=6250 391\n"));
    assert!(meta.contains("play_frame_len=7000\n"));
    let (fields, _) = parse(&meta).unwrap();
    let (restored, params) = geometry(&fields).unwrap();
    assert_eq!(restored.playback, s.playback);
    assert_eq!(params, p);
    assert_eq!(seq::frame_bytes(&restored, &params), 28_016);

    for corrupted in [
        meta.replace("play_window=6250 391", "play_window=6250 392"),
        meta.replace("play_inferred=true", "play_inferred=false"),
        meta.replace("play_frame_len=7000", "play_frame_len=6250"),
    ] {
        let (fields, _) = parse(&corrupted).unwrap();
        assert!(geometry(&fields).is_err());
    }
}
