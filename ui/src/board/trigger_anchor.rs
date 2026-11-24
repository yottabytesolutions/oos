// SPDX-License-Identifier: GPL-2.0-or-later

/// Hardware trigger anchor within the samples of a frame. The fractional part is kept as stored points over k.
#[derive(Clone, Copy, Default)]
pub(crate) struct TriggerAnchor {
    remainder: u16,
    divisor: u16,
    at_adc_rate: bool,
    cropped: bool,
}

impl TriggerAnchor {
    pub(crate) const fn new(at_adc_rate: bool) -> Self {
        Self { remainder: 0, divisor: 1, at_adc_rate, cropped: false }
    }

    pub(crate) fn cropped_play(self, word: usize, k: usize) -> Self {
        Self { remainder: (word % k) as u16, divisor: k as u16, cropped: true, ..self }
    }

    pub(crate) fn point(self, pre: usize) -> f64 {
        if self.remainder == 0 {
            pre as f64
        } else {
            pre as f64 + f64::from(self.remainder) / f64::from(self.divisor)
        }
    }

    /// At the full ADC rate the hardware index already identifies the trigger sample or pair.
    pub(crate) fn at_adc_rate(self) -> bool {
        self.at_adc_rate
    }

    pub(crate) fn cropped(self) -> bool {
        self.cropped
    }

    #[cfg(test)]
    pub(crate) fn stored_points(self, pre: usize) -> usize {
        pre * usize::from(self.divisor.max(1)) + usize::from(self.remainder)
    }
}

/// Geometry of the last completed batch, independently of raw replay.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlayedWindow {
    pub(crate) lo: u32,
    pub(crate) n: u32,
    pub(crate) k: u32,
}

#[derive(Default)]
pub(crate) struct ReadGeometry {
    /// The applied batch can replay a raw deep-measurement window.
    pub(crate) deep_ok: bool,
    pub(crate) play: Option<(oos_core::change::Identity, PlayedWindow)>,
}
