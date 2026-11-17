// SPDX-License-Identifier: GPL-2.0-or-later

use crate::board::Frame;
use std::{ops::Range, sync::Arc};

/// A math job shares acquisition storage or an already owned computed/reference signal.
pub(crate) enum Samples {
    Owned(Arc<Vec<f32>>),
    Frame { frame: Arc<Frame>, channel: usize, window: Range<usize> },
}
impl Samples {
    pub(crate) fn as_slice(&self) -> &[f32] {
        match self {
            Self::Owned(v) => v,
            Self::Frame { frame, channel, window } => &frame.ch[*channel][window.clone()],
        }
    }
}
impl std::ops::Deref for Samples {
    type Target = [f32];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
impl From<Arc<Vec<f32>>> for Samples {
    fn from(v: Arc<Vec<f32>>) -> Self {
        Self::Owned(v)
    }
}
