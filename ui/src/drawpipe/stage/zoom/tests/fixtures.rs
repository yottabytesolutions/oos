// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    pixels::{reference_picture_at, row_difference},
    *,
};
use crate::board::dho::hw::header::{dispatch_fixture, for_each_fixture, Fixture};

#[test]
#[ignore = "requires captured zoom batches in OOS_FIXTURES"]
fn captured_zoom_detail_matches_the_exact_screen_within_one_row() {
    for_each_fixture(&mut verify);
}

fn verify(fixture: &Fixture) {
    let (m, size, interp, zoom) = fixture.view().unwrap().inputs();
    let zoom = zoom.expect("a zoom fixture carries a detail view");
    let id = oos_core::change::Identity::default();
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    pipe.publish_picture_view(m, size, interp, id, Some(zoom));
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    oos_hal::sync::lock(&pipe.workers).sender = Some(send);
    dispatch_fixture(fixture, &pipe, None);
    let Message::Batch(batch) = receive.recv().unwrap() else { panic!("missing captured batch") };
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let (w, h) = zoom.size();
    let mut model = view.m.clone();
    for channel in &mut model.ch {
        zoom.transform(channel);
    }
    let tdiv = tscales()[model.zscale];
    let mut actual = Phosphor::new(w, h);
    let (mut main, mut detail) = (Default::default(), Default::default());
    let mut previous = None;
    let mut refine = Duration::ZERO;
    let mut scratch = Frame::default();
    let mut decoded = Frame::default();
    let mut max = 0;
    let range = batch.next.load(Ordering::Relaxed)..batch.frames;
    for i in range.clone() {
        let bytes = chunk(&batch, i);
        let (overview, plan, _, ti) = place_codes(
            &batch,
            &view,
            (i, bytes),
            &mut scratch,
            (&mut main, &mut detail),
            &mut refine,
            (None, previous),
        )
        .expect("normal capture supplies raw code words");
        let (_, trace) = plan.plan.raw().expect("detail is reduced before tiles");
        batch.decode.decode(i, bytes, &mut decoded);
        let expected =
            reference_picture_at(&model, &decoded, (tdiv, model.zdelay), (trace.trigger_at, trace.trigger_at), (w, h));
        let mut bands = DiffBand::bands((w, h), model.ch.each_ref().map(|channel| channel.on));
        actual.clear();
        for band in &mut bands {
            band.paint(kernel::SamplesRef::Volts(&[]), &plan.plan);
            band.fold_picture([None, None], &mut actual);
        }
        let difference = row_difference(&actual, &expected, w, h);
        assert!(difference <= 1, "{} record {i} (trigger {ti}): {difference} rows", fixture.name());
        max = max.max(difference);
        previous = plan.plan.recycle(&mut detail);
        overview.plan.recycle(&mut main);
    }
    println!("{}: {} records, max difference {max} rows", fixture.name(), range.len());
}
