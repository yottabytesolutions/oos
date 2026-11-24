// SPDX-License-Identifier: GPL-2.0-or-later
//! A deep original acquisition with a legal small live window and multiple real ring records.
use super::*;
use crate::{
    drawpipe,
    model::{tscales, Model},
    sparse::Interp,
    wave::Phosphor,
};
struct DeepWindow {
    settings: Settings,
    params: hz::Params,
    model: Model,
    bytes: Vec<u8>,
    frame_bytes: usize,
    raw_count: usize,
}
fn fixture() -> DeepWindow {
    let (mut settings, _) = setup(15, false, 3520);
    settings.tdiv = 1e-6;
    let params = hz::prep(15, 1_000_000_000, 2_000_000, 0);
    let settings = super::super::super::hw::batch_plan(&settings, &params, 1);
    let visible = seq::window(&settings, &params).0;
    let raw_count = (settings.frames as usize).min(4);
    assert!(params.memsel > crate::compress::MAX_DEPTH && raw_count >= 3);
    assert!(visible < params.memsel && seq::compress_k(&settings, &params) == 1);
    let mut window = params;
    window.memsel = visible;
    let bytes: Vec<u8> = (0..raw_count).flat_map(|i| record(&settings, &window, i)).collect();
    let frame_bytes = seq::frame_bytes(&settings, &params);
    assert_eq!(bytes.len(), raw_count * frame_bytes);
    let mut model = Model {
        tscale: tscales().iter().position(|&t| (t as f64 - 1e-6).abs() < 1e-9).unwrap(),
        trig_src: 1,
        ..Default::default()
    };
    for ch in &mut model.ch {
        ch.on = true;
    }
    DeepWindow { settings, params, model, bytes, frame_bytes, raw_count }
}
fn metadata_only_is_not_a_decoded_deep_record(w: &DeepWindow) {
    let mut meta = Frame::default();
    assert!(super::super::super::hw::codes_into(
        &w.bytes[w.frame_bytes..2 * w.frame_bytes],
        &w.settings,
        &w.params,
        0,
        &mut meta
    )
    .is_some());
    assert!(crate::deepmeas::deep_raw_frame(&meta) && meta.ch.iter().all(Vec::is_empty));
    let matcher =
        BatchMatch::prepare(&w.bytes, w.frame_bytes, 1..w.raw_count, &w.settings, &w.params, &mut Cores::default());
    let (_, codes) = BatchMatch::dispatch(Some(matcher), w.settings.clone(), w.params, (HeaderProbe::Off, None));
    assert!(
        codes.is_none(),
        "original-depth policy withholds metadata-only corrected callback before worker admission"
    );
}

fn oracle(w: &DeepWindow) -> (Phosphor, Frame) {
    let cores = seed(&w.bytes[w.frame_bytes..2 * w.frame_bytes], &w.settings, &w.params);
    let mut want = Phosphor::new(1024, 480);
    let mut last = None;
    for chunk in w.bytes.chunks_exact(w.frame_bytes).skip(1) {
        let f = frozen(chunk, &w.settings, &w.params, &cores);
        assert!(crate::deepmeas::deep_raw_frame(&f));
        drawpipe::draw_frame(&mut want, &w.model, &f, Interp::Exact, None);
        last = Some(f);
    }
    assert!(want.maps().iter().any(|c| c.iter().any(|&v| v != 0)));
    (want, last.unwrap())
}
#[test]
fn non_newest_deep_window_keeps_decoded_record_pixels_and_exact_count() {
    let _scope = drawpipe::test_scope::Scope::new();
    let w = fixture();
    metadata_only_is_not_a_decoded_deep_record(&w);
    let (want, last) = oracle(&w);
    let counters = Box::leak(Box::new(drawpipe::Counters::default()));
    let pipe = drawpipe::Pipe::start(4, 0, counters);
    pipe.publish_view(&w.model, 1024, 480, Interp::Exact, Default::default());
    let mut cores = seed(&w.bytes[w.frame_bytes..2 * w.frame_bytes], &w.settings, &w.params);
    let newest = Frame { seq: w.raw_count as u64, ..Frame::default() };
    feed(
        &pipe,
        (Box::new(Bytes(w.bytes)), w.frame_bytes, 1..w.raw_count),
        (Default::default(), std::time::Instant::now()),
        (w.settings.clone(), w.params, HeaderProbe::Off),
        Some(&mut cores),
        (Some(newest), None, true),
    );
    finish(&pipe);
    let mut got = Phosphor::new(1024, 480);
    let accepted = pipe.accept(&mut got, &mut false, &mut [0; 4], &mut (0., 0.));
    assert_eq!(accepted.frames, (w.raw_count - 1) as u64);
    assert_eq!(got.maps(), want.maps());
    assert_eq!(pipe.take_waveform((1024, 480)).unwrap().ch, last.ch);
    assert_eq!(counters.routes.count(drawpipe::route::Route::Dense), 0);
    println!(
        "DEEP_WINDOW original={} visible={} planned_raw={} actual_raw={} kept={} bytes_per_record={}",
        w.params.memsel,
        seq::window(&w.settings, &w.params).0,
        w.settings.frames,
        w.raw_count,
        w.raw_count - 1,
        w.frame_bytes
    );
}
