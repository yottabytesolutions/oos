// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes, FieldDiff, Request};
use crate::model::Model;

/// Only an uninterrupted chain of driver writes can replace the ordinary position classification.
pub(super) fn matches(a: &Model, b: &Model, requests: &[Request], zoomed: bool) -> bool {
    let position = |m: &Model| if zoomed { m.zdelay } else { m.delay }.to_bits();
    let mut current = position(a);
    let mut found = false;
    for request in requests {
        if let Request::PlaybackPosition { zoom, before, after } = *request {
            if zoom == zoomed {
                if before != current {
                    return false;
                }
                current = after;
                found = true;
            }
        }
    }
    found && current == position(b)
}

pub(super) fn fields(a: &Model, b: &Model, requests: &[Request], mut emit: impl FnMut(FieldDiff)) {
    let delay = matches(a, b, requests, false);
    let zoom = matches(a, b, requests, true);
    super::fields(a, b, |mut f| {
        if (delay && f.name == "delay") || (zoom && f.name == "zdelay") {
            f.classes = Classes::VIEW;
        }
        emit(f);
    });
}
