// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::batch::Readout;
use crate::board::dho::switches::HeaderProbe;
use crate::drawpipe::{self, route::RouteTrace, Pipe, RawBatch};
use crate::ui::lifecycle::test_wait::HANG_GUARD;
use oos_core::change::Identity;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

mod phase;
mod tests;
pub(crate) use super::format::Fixture;

impl Fixture {
    pub(crate) fn assert_route(&self, observed: &str, submitted: &std::ops::Range<usize>) {
        let (expected, range) = self.expected_route().expect("worker fixtures record their route and range");
        let (expected, recorded_wide) = expected.rsplit_once(" wide_search=").expect("recorded route has a wide count");
        let (observed, observed_wide) = observed.rsplit_once(" wide_search=").expect("observed route has a wide count");
        assert_eq!(*submitted, range, "{}: submitted record range", self.name());
        let mut expected = routes(expected);
        // Older headered captures recorded regular lines as volts.
        if self.settings().0.header {
            if let Some(volts) = expected.remove("volts") {
                *expected.entry("dense").or_default() += volts;
            }
        }
        assert_eq!(routes(observed), expected, "{}: worker placement route", self.name());
        println!("{} wide_search={observed_wide} recorded_wide_search={recorded_wide}", self.name());
    }
}

pub(crate) fn for_each_fixture(test: &mut dyn FnMut(&Fixture)) {
    let Some(root) = std::env::var_os("OOS_FIXTURES").map(PathBuf::from) else {
        eprintln!("OOS_FIXTURES is unset; skipping captured batches");
        return;
    };
    let paths = fixture_paths(&root);
    assert!(!paths.is_empty(), "OOS_FIXTURES contains no completed fixtures");
    for path in paths {
        let fixture = Fixture::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        test(&fixture);
    }
}

fn fixture_paths(root: &Path) -> Vec<PathBuf> {
    if root.join("fixture.txt").is_file() {
        return vec![root.to_owned()];
    }
    let mut paths: Vec<_> = root
        .read_dir()
        .expect("OOS_FIXTURES must name a fixture or fixture collection")
        .map(|entry| entry.expect("reading fixture directory").path())
        .filter(|path| path.is_dir() && path.join("fixture.txt").is_file())
        .collect();
    paths.sort();
    paths
}

struct Bytes(Arc<[u8]>);
impl RawBatch for Bytes {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        self
    }
}

pub(crate) fn dispatch_fixture(fixture: &Fixture, pipe: &Pipe, trace: Option<Box<RouteTrace>>) {
    let (s, p) = fixture.settings();
    let (bytes, (fb, count, frame0_drop), newest) = fixture.source();
    let mut bytes = Some(Bytes(bytes));
    let mut cores = fixture.cores();
    let id = Identity::default();
    let readout = Readout::new(
        pipe,
        (s, p),
        (fb, count, frame0_drop),
        (count as u64, HeaderProbe::Off, newest),
        (id, Instant::now()),
    )
    .trace(trace);
    let returned = readout.dispatch(
        &mut bytes,
        fixture.matching().then_some(&mut cores),
        |mut f| {
            if let Some(la) = fixture.la() {
                f.la = Some(la.clone());
            }
            f
        },
        &|| false,
    );
    assert!(bytes.is_none() && returned.is_empty(), "fixture DMA must be handed to the placement worker");
}

pub(crate) fn replay_route(fixture: &Fixture) -> bool {
    let Some(view) = fixture.view() else {
        eprintln!("{}: batch did not reach worker placement; skipping placement replay", fixture.name());
        return false;
    };
    let (m, size, interp, zoom) = view.inputs();
    let counters = Box::leak(Box::default());
    let (workers, prior_hits) = view.scheduling();
    let pipe = Pipe::start(workers, 0, counters);
    pipe.publish_picture_view(m, size, interp, Identity::default(), zoom);
    let (send, receive) = std::sync::mpsc::channel();
    let mut trace = RouteTrace::new(move |observed| {
        send.send((observed.route().to_owned(), observed.range())).expect("replay is waiting for the batch");
    });
    trace.set_replay_prior_hits(prior_hits);
    dispatch_fixture(fixture, &pipe, Some(trace));
    drawpipe::host::finish_batch(&pipe, 0).expect("wait for the returned fixture DMA buffer");
    let (got, submitted) = receive.recv().expect("buffer return signals the trace is complete");
    assert_eq!(drawpipe::shutdown::close(&pipe, Instant::now() + HANG_GUARD), 0);
    fixture.assert_route(&got, &submitted);
    true
}

fn routes(report: &str) -> std::collections::BTreeMap<&str, u64> {
    report
        .split_ascii_whitespace()
        .skip(1)
        .filter_map(|field| {
            let (route, value) = field.split_once('=').unwrap();
            let records = value.parse().unwrap();
            (records > 0).then_some((route, records))
        })
        .collect()
}
