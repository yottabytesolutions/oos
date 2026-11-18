// SPDX-License-Identifier: GPL-2.0-or-later
//! Draw pipeline selection from startup configuration.

/// The pipeline mode: `batch` (this pipeline) or `frames` (every frame through the UI queue).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Frames,
    #[default]
    Batch,
}

/// `env` if set, else the value of `key` in system.conf text `conf` (a fork key: kept by `SysConf` as an unknown line).
pub fn conf_value(env: Option<&str>, conf: &str, key: &str) -> Option<String> {
    let conf_val = conf
        .lines()
        .filter_map(|l| l.split('#').next()?.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().to_string());
    env.map(str::to_string).or(conf_val)
}

/// OOS_PIPELINE, else the `acq.pipeline` line of system.conf, else Batch. Invalid values log and use the default.
pub fn mode_from(env: Option<&str>, conf: &str) -> Mode {
    match conf_value(env, conf, "acq.pipeline").as_deref() {
        Some("batch") => Mode::Batch,
        Some("standard" | "frames") => Mode::Frames,
        None => Mode::default(),
        Some(value) => {
            eprintln!("oos-ui: invalid acq.pipeline {:?}; using default batch", value);
            Mode::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_defaults_to_batch() {
        assert_eq!(mode_from(None, ""), Mode::Batch);
        assert_eq!(mode_from(None, "ntp = off\n"), Mode::Batch);
        let image = include_str!("../../../rootfs/initramfs/etc/oos/system.conf");
        assert_eq!(mode_from(None, image), Mode::Batch);
    }

    #[test]
    fn pipeline_mode_from_env_and_system_conf() {
        assert_eq!(mode_from(None, "ntp = off\nacq.pipeline = batch   # new path\n"), Mode::Batch);
        for value in ["standard", "frames"] {
            assert_eq!(mode_from(None, &format!("acq.pipeline = {} # comparison\n", value)), Mode::Frames);
            assert_eq!(mode_from(Some(value), "acq.pipeline = batch\n"), Mode::Frames);
            assert_eq!(mode_from(Some("batch"), &format!("acq.pipeline = {}\n", value)), Mode::Batch);
        }
        assert_eq!(mode_from(Some("batch"), ""), Mode::Batch);
    }

    #[test]
    fn invalid_pipeline_uses_the_default() {
        for value in ["bogus", ""] {
            assert_eq!(mode_from(None, &format!("acq.pipeline = {}\n", value)), Mode::Batch);
            assert_eq!(mode_from(Some(value), "acq.pipeline = standard\n"), Mode::Batch);
        }
    }
}
