// SPDX-License-Identifier: GPL-2.0-or-later

pub(crate) mod deposit;

use crate::drawpipe::{Accepted, Counters, Stats};
use crate::wave::Phosphor;
use deposit::Deposit;
use oos_core::change::Identity;

/// Fold a completed deposit into the UI phosphor until picture ownership moves in F7.
pub(crate) fn fold_deposit(
    deposit: &mut Deposit,
    ph: &mut Phosphor,
    clear: &mut bool,
    stats: Stats<'_>,
    identity: Option<Identity>,
    counters: &Counters,
) -> Accepted {
    if deposit.records == 0 {
        return Accepted::default();
    }
    if *clear {
        ph.clear();
        *clear = false;
    }
    for band in &mut deposit.planes {
        band.fold(&mut ph.map[band.channel]);
    }
    for (a, b) in stats.edges.iter_mut().zip(deposit.edges) {
        *a += b;
    }
    *stats.off = (stats.off.0 + deposit.off.0, stats.off.1.max(deposit.off.1));
    let folds = if deposit.identity == identity {
        std::mem::take(&mut deposit.folds)
    } else {
        Counters::add(&counters.losses.folds, deposit.folds.iter().map(|(_, f)| f.n).sum());
        Vec::new()
    };
    Accepted {
        frames: deposit.records,
        triggered: deposit.triggered,
        folds,
        newest: deposit.read_at,
        stale: deposit.progress.stale,
        waveform: None,
    }
}
