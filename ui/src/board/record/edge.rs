// SPDX-License-Identifier: GPL-2.0-or-later

use super::{smoothed_edge, trigger_fits, Frame};
use crate::model::{Model, ModelExt, Slope, TrigCoup, TrigType};

pub(super) fn event_bounds(f: &Frame, m: &Model, window: f64, len: usize) -> ((usize, usize), (usize, usize)) {
    let screen = (window / f.dt).ceil() as usize;
    // The hardware trigger already put the event near `pre`. Allow a tenth of a screen plus the
    // time the trigger condition needs before its decision (pulse widths, timeouts, Nth-edge idle time).
    let near = (screen / 10).max(2000);
    let span = (m.trig_span() / f.dt).ceil().max(0.0) as usize;
    let bounds = if m.trig_type == TrigType::Video {
        (0, len)
    } else {
        (f.pre.saturating_sub(near + span), (f.pre + near).min(len))
    };
    (bounds, (near, screen / 2))
}

pub(super) fn event_near(
    f: &Frame,
    m: &Model,
    (lo, search): (usize, (usize, usize)),
    (window, first, len): (f64, f64, usize),
    input: &oos_dsp::trigger::Input<'_>,
) -> Option<f64> {
    if m.trig_type == TrigType::Edge {
        let source = &f.ch[m.trig_src];
        let hi = lo + input.ch[m.trig_src].len();
        return edge_event(f, m, (lo, hi), search, (window, first, len, source.len()), |range| {
            source[range].iter().copied()
        });
    }
    let fits = trigger_fits(f.dt, (window, first, len));
    let distance = |t: f64| (t - f.pre as f64).abs();
    let all = crate::trig::find_all(m, input);
    all.iter()
        .map(|t| t + lo as f64)
        .filter(|&t| fits(t))
        .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
        .or_else(|| (!all.is_empty()).then_some(f.pre as f64))
}

fn edge_event<I: Iterator<Item = f32>>(
    f: &Frame,
    m: &Model,
    (lo, hi): (usize, usize),
    (near, half_screen): (usize, usize),
    (window, first, len, source_len): (f64, f64, usize, usize),
    samples: impl Fn(std::ops::Range<usize>) -> I,
) -> Option<f64> {
    let fits = trigger_fits(f.dt, (window, first, len));
    let distance = |t: f64| (t - f.pre as f64).abs();
    let hyst = m.trig_hyst_div() * m.ch[m.trig_src].volts_per_div();
    let nearest = |base: usize, end: usize| {
        let mut found = None;
        let mut any = false;
        oos_dsp::edges::visit_edges_of(samples(base..end), m.trig_level, hyst, |edge| {
            if m.slope == Slope::Either || edge.rising == (m.slope == Slope::Rise) {
                any = true;
                let time = base as f64 + edge.t;
                if fits(time) && found.is_none_or(|old| distance(time).total_cmp(&distance(old)).is_lt()) {
                    found = Some(time);
                }
            }
        });
        (found, any)
    };
    let (found, any) = nearest(lo, hi);
    if found.is_some() || near >= half_screen {
        return found.or(any.then_some(f.pre as f64));
    }
    let first = f.pre.saturating_sub(half_screen);
    let end = (f.pre + half_screen).min(len);
    let (wide, any_wide) = if end <= source_len { nearest(first, end) } else { (None, false) };
    wide.or((any || any_wide).then_some(f.pre as f64))
}

/// Average uses the record trigger's screen-fit and edge rules with the Normal code-backed search.
/// `None` selects the record decoder for trigger types that require other channels or logic samples.
pub(crate) fn record_trigger_codes(
    f: &Frame,
    m: &Model,
    geometry: (usize, f64, f64),
    source: &crate::fastdraw::Codes<'_>,
    len: usize,
) -> Option<(f64, bool)> {
    use crate::fastdraw::Samples;
    if f.roll || f.scan {
        return None;
    }
    if let Some(ti) = f.fixed_ti {
        return Some((ti, true));
    }
    if m.trig_type != TrigType::Edge || m.trig_line().is_some() || source.len() < 2 {
        return None;
    }
    let deep = crate::deepmeas::deep_raw_frame(f);
    let filtered = m.trig_coupling() != TrigCoup::Dc;
    let (smooth, centered) = if !deep && !filtered && (256..=8192).contains(&source.len()) {
        code_edge_can_cross(source, m)
    } else {
        (true, true)
    };
    let found = smooth.then(|| smoothed_edge(f, m, source, len, geometry, deep)).flatten().or_else(|| {
        if !centered {
            return None;
        }
        let (_, window, first) = geometry;
        let (bounds, search) = event_bounds(f, m, window, len);
        edge_event(f, m, bounds, search, (window, first, len, source.len()), |range| range.map(|i| source.at(i)))
    });
    Some((found.unwrap_or(f.pre as f64), found.is_some()))
}

fn code_edge_can_cross(source: &crate::fastdraw::Codes<'_>, m: &Model) -> (bool, bool) {
    let level = m.trig_level;
    let hyst = m.trig_hyst_div() * m.ch[m.trig_src].volts_per_div();
    if !level.is_finite() || !hyst.is_finite() || hyst < 0.0 {
        return (true, true);
    }
    let Some((lo, hi)) = crate::trignear::code_trigger_bounds(source) else { return (true, true) };
    let rising = m.slope != Slope::Fall && lo < level - hyst && hi >= level;
    let falling = m.slope != Slope::Rise && hi > level + hyst && lo <= level;
    (rising || falling, lo < level - hyst / 2.0 && hi > level + hyst / 2.0)
}
