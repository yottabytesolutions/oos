// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker
use std::time::Duration;

/// Times of the last frame: copying into the arena here; upload, draw and read back in the helper; and the whole
/// wait for its answer. Values are milliseconds except for the upload duration.
#[derive(Clone, Copy, Default, Debug)]
pub struct Times {
    pub copy: f32,
    pub upload: Duration,
    pub draw: f32,
    pub read: f32,
    /// how long `collect` waited
    pub wait: f32,
    /// from sending to the answer
    pub latency: f32,
    /// the helper's whole time (upload and draw) of the last frame of the panel and of the monitor
    pub panel: f32,
    pub monitor: f32,
}
impl Times {
    pub fn upload_ms(&self) -> f32 {
        self.upload.as_secs_f32() * 1e3
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn with_helper(self, done: super::DoneMsg) -> Result<Self, String> {
        let upload = Duration::try_from_secs_f32(done.ms_upload * 1e-3)
            .map_err(|error| format!("invalid helper upload time {} ms: {error}", done.ms_upload))?;
        Ok(Self { upload, draw: done.ms_draw, read: done.ms_read, ..self })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::DoneMsg;

    #[test]
    fn helper_upload_time_is_validated_before_recording() {
        let times = Times { copy: 3.0, ..Default::default() };
        for ms_upload in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
            let error = times.with_helper(DoneMsg { ms_upload, ..Default::default() }).unwrap_err();
            assert!(error.starts_with("invalid helper upload time"), "{error}");
        }
        for (ms_upload, expected) in [(0.0, Duration::ZERO), (125.0, Duration::from_millis(125))] {
            let actual =
                times.with_helper(DoneMsg { ms_upload, ms_draw: 2.0, ms_read: 4.0, ..Default::default() }).unwrap();
            assert_eq!(actual.upload, expected);
            assert_eq!(actual.upload_ms(), ms_upload);
            assert_eq!((actual.copy, actual.draw, actual.read), (3.0, 2.0, 4.0));
        }
    }
}
