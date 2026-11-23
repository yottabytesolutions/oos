// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Step, Write};

pub(super) fn writes(st: &[Step]) -> Vec<Write> {
    st.iter().filter_map(|s| if let Step::Wr(w) = s { Some(*w) } else { None }).collect()
}
