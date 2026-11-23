// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn every_table_row_round_trips_its_value() {
    let model = super::super::codec_tests::populated();
    for section in SECTIONS {
        let (keys, count) = match section {
            Plain(keys) => (*keys, 1),
            Indexed { keys, count, .. } => (*keys, *count),
            Maths => (math::MATH_CHANNEL, crate::math::NMATH),
            Decoders => (decoder::DECODER, 1),
        };
        for i in 0..count {
            for key in keys {
                let value = (key.read)(&model, i).unwrap_or_else(|| panic!("{}[{i}] omitted", key.name));
                let mut restored = Model::default();
                (key.apply)(&mut restored, i, &value).unwrap_or_else(|e| panic!("{}: {e}", key.name));
                assert_eq!((key.read)(&restored, i), Some(value), "{}[{i}]", key.name);
            }
        }
    }
}

#[test]
fn read_only_table_rows_preserve_legacy_mappings() {
    let mut model = Model::default();
    set(&mut model, "cur_b_ch", "2").unwrap();
    assert_eq!(model.cur_b_source, Some(WaveId::Ch(2)));
    set(&mut model, "tscale", "0").unwrap();
    set(&mut model, "zscale", "0").unwrap();
    assert_eq!((tscales()[model.tscale], tscales()[model.zscale]), (5e-9, 5e-9));
    set(&mut model, "hdmi_wave", "true").unwrap();
    assert_eq!(model.hdmi, HdmiMode::Waveform);
    for i in 0..NCH {
        set(&mut model, &format!("ch{}.vscale", i + 1), "0").unwrap();
        assert_eq!(model.ch[i].vscale, 2);
        set(&mut model, &format!("ch{}.bw20", i + 1), "true").unwrap();
        assert_eq!(model.ch[i].bw, BW_20M);
    }
}

#[test]
fn retired_draw_path_is_accepted_and_omitted_when_saved() {
    assert!(is_retired_setup_key("draw_path"));
    assert!(!is_retired_setup_key("draw_paths"));
    for value in ["batch", "standard"] {
        let model = parse_setup(&format!("draw_path = {value}\n")).unwrap();
        assert!(!setup_text(&model).contains("draw_path"));
    }
}

#[test]
fn decoder_slot_accepts_legacy_logic_analyser_inputs() {
    let model = parse_setup("d2.dec_la = 1 0 2 - -\n").unwrap();
    let decoder = model.dec_view(1);
    assert!(decoder.dec_la_in);
    assert_eq!(decoder.dec_la, [Some(0), Some(2), None, None]);
}

#[test]
fn setup_file_stores_decoder_slot_with_only_unknown_fields() {
    let model = parse_setup("d2.future_key = ignored\n").unwrap();
    assert!(model.decs[1].is_some());
}
