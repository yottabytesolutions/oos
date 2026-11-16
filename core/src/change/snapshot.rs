// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Model, WaveId};

/// Retained snapshot capacity, including all nested String/Vec payloads, rather than live lengths alone.
pub fn snapshot_bytes(m: &Model) -> usize {
    std::mem::size_of::<Model>()
        + m.meas.capacity() * std::mem::size_of::<(crate::model::Meas, WaveId)>()
        + m.meas2.capacity() * std::mem::size_of::<(usize, WaveId, WaveId)>()
        + m.cat.capacity() * std::mem::size_of::<(u16, WaveId, WaveId, WaveId, WaveId)>()
        + m.bus.label.capacity()
        + m.ch_label.iter().map(String::capacity).sum::<usize>()
        + m.math_expr.iter().map(String::capacity).sum::<usize>()
        + m.view_slots.iter().map(String::capacity).sum::<usize>()
        + m.bode_addr.capacity()
        + m.zones.iter().map(|z| z.pts.capacity() * std::mem::size_of::<(f64, f64)>()).sum::<usize>()
}

// clone_from can grow a Vec/String geometrically. Reserve a conservative capacity bound before mutation,
// including retained capacities in other fields when the active payload moves between slots.
pub(super) fn projected_bytes(a: &Model, b: &Model) -> usize {
    let grow = |capacity: usize, length: usize| {
        if length <= capacity {
            capacity
        } else {
            length.max(capacity.saturating_mul(2)).max(8)
        }
    };
    let strings = a
        .ch_label
        .iter()
        .chain(&a.math_expr)
        .chain(&a.view_slots)
        .chain(std::iter::once(&a.bode_addr))
        .zip(b.ch_label.iter().chain(&b.math_expr).chain(&b.view_slots).chain(std::iter::once(&b.bode_addr)))
        .map(|(a, b)| grow(a.capacity(), b.len()))
        .sum::<usize>();
    std::mem::size_of::<Model>()
        + strings
        + grow(a.meas.capacity(), b.meas.len()) * std::mem::size_of::<(crate::model::Meas, WaveId)>()
        + grow(a.meas2.capacity(), b.meas2.len()) * std::mem::size_of::<(usize, WaveId, WaveId)>()
        + grow(a.cat.capacity(), b.cat.len()) * std::mem::size_of::<(u16, WaveId, WaveId, WaveId, WaveId)>()
        + grow(a.bus.label.capacity(), b.bus.label.len())
        + a.zones.iter().zip(&b.zones).map(|(a, b)| grow(a.pts.capacity(), b.pts.len()) * 16).sum::<usize>()
}
