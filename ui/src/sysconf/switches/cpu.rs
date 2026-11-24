// SPDX-License-Identifier: GPL-2.0-or-later

use super::{conf_value, text, Env};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum GpuCpu {
    #[default]
    Ui,
    Hw,
}

impl GpuCpu {
    pub(super) fn parse(env: Env, conf: &str) -> Self {
        match conf_value(text(env, "OOS_GPU_CPU").as_deref(), conf, "gpu.cpu").as_deref() {
            Some("hw") => Self::Hw,
            Some("ui") | None => Self::Ui,
            Some(value) => {
                eprintln!("oos-ui: invalid gpu.cpu {value:?}; using default ui");
                Self::Ui
            }
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Hw => "hw",
        }
    }
}

#[cfg(test)]
#[test]
fn gpu_cpu_uses_environment_before_config_and_defaults_to_ui() {
    assert_eq!(GpuCpu::parse(&|_| None, ""), GpuCpu::Ui);
    assert_eq!(GpuCpu::parse(&|_| None, "gpu.cpu = hw"), GpuCpu::Hw);
    assert_eq!(GpuCpu::parse(&|_| Some("ui".into()), "gpu.cpu = hw"), GpuCpu::Ui);
    assert_eq!(GpuCpu::parse(&|_| None, "gpu.cpu = little"), GpuCpu::Ui);
}
