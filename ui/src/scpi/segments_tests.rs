// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn segment_times_during_recording_answers_empty_state() {
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::for_tests(crate::wave::Source::Hw { shared: shared.clone(), seen: 0 });
    oos_hal::sync::lock(&shared).seg.phase = crate::board::SegPhase::Recording;
    let mut scpi = Scpi::open(0, false);
    let canvas = Canvas::new(1, 1);
    scpi.exec_line(&mut ui, &canvas, ":BODE:GEN \"\"");
    assert_eq!(scpi.response(&mut ui, &canvas, ":ACQuire:SEGMents:TIMes?"), Some(b"\n".to_vec()));
    assert_eq!(scpi.response(&mut ui, &canvas, ":BODE:GEN?"), Some(b"\n".to_vec()));
    assert_eq!(scpi.response(&mut ui, &canvas, "*OPC?"), Some(b"1\n".to_vec()));
    assert_eq!(scpi.response(&mut ui, &canvas, ":ACQ:SEGM:TIM?;*OPC?"), Some(b";1\n".to_vec()));
    assert_eq!(scpi.response(&mut ui, &canvas, "*CLS"), None);
}
