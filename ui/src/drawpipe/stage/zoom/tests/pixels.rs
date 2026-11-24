// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn zoom_matches_the_decoded_main_and_detail_pixels_without_allocating_after_warmup() {
    for channels in [1, 4] {
        for tdiv in [1e-6, 50e-9, 5e-9] {
            let mut m = model(channels);
            m.zdelay = tdiv * 0.37;
            m.deskew_ch = [0.0, 1.3e-9, -2.1e-9, 31.3e-9];
            for dots in [DotsMode::Off, DotsMode::On] {
                m.dots = dots;
                compare_pixels(&m, tdiv);
            }
        }
    }
    compare_pixels(&model(4), 5e-9);
}

fn compare_pixels(m: &Model, zoom_div: f32) {
    let (b, frame) = batch(m);
    let mut v = view(m.clone(), Some(zoom_div));
    Arc::get_mut(&mut v).unwrap().zoom = Some(route::ZoomView::new((WIDTH, HEIGHT), 1.7, 0.16));
    let mut refine = Duration::ZERO;
    assert!(place_codes(
        &b,
        &v,
        (0, chunk(&b, 0)),
        &mut Frame::default(),
        (&mut Default::default(), &mut Default::default()),
        &mut refine,
        (None, None),
    )
    .is_some());
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let mut painter = Painter::new(1, 0);
    let used = m.ch.each_ref().map(|c| c.on);
    let mut deposit = pipe.slot.acquire_picture(v.gen, v.picture_sizes(), None, used).unwrap();
    let mut paint = |deposit: &mut Deposit| {
        let count = painter.place_slice(&b, &v, 0);
        painter.draw_tiles(&b, deposit, count);
    };
    paint(&mut deposit);
    deposit.reset(None);
    let (allocations, ()) = crate::system::test_allocator::count_own(|| paint(&mut deposit));
    assert_eq!(allocations, 0, "zoom {zoom_div} {:?}", m.dots);

    let (mut main, mut detail) = (Phosphor::new(WIDTH, HEIGHT), Phosphor::new(WIDTH, HEIGHT));
    deposit.sort_bands();
    deposit.blend_picture(&mut PerMap::new(&mut main, Some(&mut detail)), true);
    let span = f64::from(m.time_per_div()) * f64::from(HDIV);
    let first = f64::from(m.delay) - span / 2.0;
    // Compare the rasterizers at the main view's accepted trigger, including the legacy deskew alignment.
    let dense = dense_placement(&b, &v);
    let ti = dense.plan.raw().unwrap().1.trigger_at;
    let (i0, _, t0) = crate::board::record::raw_window(ti, first, span, frame.dt, frame.ch[0].len());
    let aligned = -(t0 - i0 as f64 * frame.dt) / frame.dt;
    let main_reference = reference_picture(m, &frame, (m.time_per_div(), m.delay), (ti, aligned));
    let mut detail_model = m.clone();
    for channel in &mut detail_model.ch {
        v.zoom.unwrap().transform(channel);
    }
    let detail_reference = reference_picture(&detail_model, &frame, (zoom_div, m.zdelay), (aligned, aligned));
    assert!(main_reference.channel_max(0) > 0 && detail_reference.channel_max(0) > 0);
    assert_eq!(main.maps(), main_reference.maps(), "overview for zoom {zoom_div} {:?}", m.dots);
    if m.dots_at(WIDTH as f64 / crate::sparse::POINTS as f64) {
        assert_eq!(detail.maps(), detail_reference.maps(), "dots for zoom {zoom_div}");
    } else if f64::from(zoom_div) * f64::from(HDIV) < 2.0 * WIDTH as f64 * frame.dt {
        assert!(row_difference(&detail, &detail_reference, WIDTH, HEIGHT) <= 1, "zoom {zoom_div}");
    } else {
        assert_eq!(detail.maps(), detail_reference.maps(), "dense window at zoom {zoom_div}");
    }
}

pub(super) fn row_difference(actual: &Phosphor, expected: &Phosphor, width: usize, height: usize) -> usize {
    let bounds = |map: &[u16], x: usize| {
        let mut rows = map.chunks_exact(width).enumerate().filter_map(|(y, row)| (row[x] != 0).then_some(y));
        rows.next().map(|first| (first, rows.next_back().unwrap_or(first)))
    };
    let mut max = 0;
    for (a, b) in actual.maps().iter().zip(expected.maps()) {
        for x in 0..width {
            let difference = match (bounds(a, x), bounds(b, x)) {
                (Some((lo, hi)), Some((first, last))) => lo.abs_diff(first).max(hi.abs_diff(last)),
                (None, Some((lo, hi))) | (Some((lo, hi)), None) => (hi + 1).min(height - lo),
                (None, None) => 0,
            };
            max = max.max(difference);
        }
    }
    max
}

pub(super) fn reference_picture(m: &Model, frame: &Frame, view: (f32, f32), (ti, aligned): (f64, f64)) -> Phosphor {
    reference_picture_at(m, frame, view, (ti, aligned), (WIDTH, HEIGHT))
}

pub(super) fn reference_picture_at(
    m: &Model,
    frame: &Frame,
    view: (f32, f32),
    (ti, aligned): (f64, f64),
    (width, height): (usize, usize),
) -> Phosphor {
    let mut ph = Phosphor::new(width, height);
    let n = crate::sparse::POINTS;
    let span = f64::from(view.0) * f64::from(HDIV);
    let dt = span / n as f64;
    let pairs = dt / frame.dt >= 2.0 && m.acq != AcqMode::HiRes;
    let mut out = Vec::new();
    for (c, values) in frame.ch.iter().enumerate().filter(|(c, _)| m.ch[*c].on) {
        let shift = m.deskew_ch[c];
        let display_delay = (f64::from(view.1) + shift) as f32;
        let first = f64::from(display_delay) - span / 2.0;
        let ti = if shift == 0.0 { ti } else { aligned };
        crate::board::record::resample_channel(values, (frame.dt, frame.envelope), m.acq, (n, dt, first), ti, &mut out);
        ph.draw_ex(c, m, view, first - shift, dt, &out, 24, None, pairs);
    }
    ph
}
