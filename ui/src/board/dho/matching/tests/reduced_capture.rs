// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    drawpipe::{self, route::Route},
    model::{tscales, DotsMode, Model},
    picture::deposit::standard,
    sparse::Interp,
    wave::Phosphor,
};

#[test]
fn reduced_capture_uses_column_adapters_and_keeps_the_stored_words() {
    for (peak, dots, expected) in
        [(false, false, Route::Upsampled), (false, true, Route::Dots), (true, false, Route::Pairs)]
    {
        verify(peak, dots, expected);
    }
}

fn verify(peak: bool, dots: bool, expected: Route) {
    let _scope = drawpipe::test_scope::Scope::new();
    let tdiv = 20e-9;
    let mut model = Model { tscale: tscales().iter().position(|&t| t == tdiv).unwrap(), ..Default::default() };
    model.dots = if dots { DotsMode::On } else { DotsMode::Off };
    let mut s = Settings { mask: 1, tdiv: f64::from(tdiv), window: None, ..Default::default() };
    s.ch[0].scale = 1.0;
    let p = s.reduce_capture(peak, 8, true).unwrap();
    let bytes = payload(&s, &p, peak);
    let size = (256, 192);
    let counters = Box::leak(Box::new(drawpipe::Counters::default()));
    let pipe = drawpipe::Pipe::start(2, 0, counters);
    pipe.publish_view(&model, size.0, size.1, Interp::Exact, Default::default());
    let want = super::super::super::hw::decode(&bytes, &s, &p, 1);
    if peak {
        assert!(want.envelope && want.ch[0].as_chunks::<2>().0.iter().all(|pair| pair[0] < pair[1]));
        assert_eq!((want.dt, want.fixed_ti), (2.0 / p.rate as f64, Some(p.pre as f64 / 2.0)));
    }
    verify_zoom(&model, &s, &p, &bytes);
    let fb = bytes.len();
    feed(
        &pipe,
        (Box::new(Bytes(bytes)), fb, 0..1),
        (Default::default(), std::time::Instant::now()),
        (s, p, HeaderProbe::Off),
        None,
        (Some(Frame { seq: 1, ..Default::default() }), None, true),
    );
    finish(&pipe);
    let mut got = Phosphor::new(size.0, size.1);
    assert_eq!(pipe.accept(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 1);
    assert_eq!(counters.routes.count(expected), 1);
    assert!(got.maps()[0].iter().any(|&count| count != 0));
    if peak {
        let plan = standard::batch_plan(&model, size, &want, None, None);
        let mut reference = vec![0; size.0 * size.1];
        standard::fast_deposit(0, &mut reference, 0, &want, &plan.plan);
        assert!(got.maps()[0] == reference, "peak enters the pair column kernel");
    }
    let tap = pipe.take_waveform(size).unwrap();
    assert_eq!(tap.ch[0], want.ch[0]);
}

fn verify_zoom(model: &Model, s: &Settings, params: &hz::Params, bytes: &[u8]) {
    use crate::picture::deposit::envelope::Layout;
    let mut frame = Frame::default();
    let raw = super::super::super::hw::codes_into(bytes, s, params, 1, &mut frame).unwrap();
    let layout = Layout::new(&raw).unwrap();
    let channels = std::array::from_fn(|c| raw.channel(bytes, c));
    let main = standard::projection(model, 256, 192, 4.0);
    let detail = standard::projection(model, 128, 96, 2.0);
    let mut refine = std::time::Duration::ZERO;
    let (overview, zoom, _) =
        standard::zoom_reduced_plans(model, &frame, (main, detail), (channels, layout), (None, None), &mut refine)
            .unwrap();
    for plan in [overview, zoom] {
        assert_eq!(plan.route, Route::Zoom);
        let (_, trace) = plan.plan.raw().expect("reduced zoom stays in the code kernels");
        assert_eq!(trace.spans[0].is_some(), !main.dots);
    }
}

fn payload(s: &Settings, p: &hz::Params, peak: bool) -> Vec<u8> {
    let bytes: Vec<_> = (0..p.memsel)
        .flat_map(|i| {
            let word = if peak { 30_000 + 6_000 * (i % 2) } else { 30_000 + 200 * i };
            (word as u16).to_le_bytes()
        })
        .collect();
    assert_eq!(bytes.len(), seq::frame_bytes(s, p));
    bytes
}
