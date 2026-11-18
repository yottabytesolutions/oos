// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_core::change::{Identity, Request, Shadow, UiAcq};

fn identities(request: Request) -> (Model, Identity) {
    let model = Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let id = shadow.observe(&model, UiAcq::default(), &[request], &config).unwrap().identity();
    (model, id)
}

fn accumulator(gen: u64, identity: Identity) -> Acc {
    let mut a = Acc::new(gen, 8, 8, [true; NCH]);
    a.identity = Some(identity);
    a.frames = 1;
    let mut fold = crate::framestats::Fold::default();
    fold.push(123.0);
    a.folds.push(((Meas::Vavg, 0), fold));
    a
}

#[test]
fn old_folds_cannot_refill_current_statistics() {
    for request in [Request::StatsReset, Request::Clear] {
        let (model, current) = identities(request);
        let counters = Box::leak(Box::<Counters>::default());
        let pipe = Pipe::start(1, 0, counters);
        pipe.set_view(Some(&model), 8, 8, Interp::Exact);
        pipe.set_identity(current);
        let gen = pipe.view.lock().unwrap().gen;
        let mut phosphor = Phosphor::new(8, 8);
        pipe.mails[0].lock().unwrap().full.push(accumulator(gen, Identity::default()));
        let stale = pipe.merge(&mut phosphor, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert!(stale.folds.is_empty(), "late folds must not refill reset statistics");
        assert_eq!(counters.losses.pipe.load(Ordering::Relaxed), u64::from(request == Request::Clear));
        assert_eq!(counters.losses.folds.load(Ordering::Relaxed), 1);
        assert_eq!(counters.losses.overflow.load(Ordering::Relaxed), 0);
        assert_eq!(stale.frames, u64::from(request == Request::StatsReset), "statistics alone retain valid pictures");
        pipe.mails[0].lock().unwrap().full.push(accumulator(gen, current));
        let valid = pipe.merge(&mut phosphor, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(valid.frames, 1);
        assert_eq!(valid.folds[0].1.n, 1, "valid current folds are retained");
    }
}

#[test]
fn mailbox_coalescing_retains_only_current_folds_in_either_order() {
    let (_, current) = identities(Request::StatsReset);
    for newer_first in [false, true] {
        let counters = Counters::default();
        let mut old = accumulator(1, Identity::default());
        let mut new = accumulator(1, current);
        let combined = if newer_first {
            new.add(&old, &counters);
            new
        } else {
            old.add(&new, &counters);
            old
        };
        assert_eq!(counters.losses.folds.load(Ordering::Relaxed), 1);
        assert_eq!(counters.losses.pipe.load(Ordering::Relaxed), 0);
        assert_eq!(combined.frames, 2);
        assert_eq!(combined.identity, Some(current));
        assert_eq!(combined.folds[0].1.n, 1);
    }
}
