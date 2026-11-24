// SPDX-License-Identifier: GPL-2.0-or-later

use crate::sysconf::{switches::GpuCpu, Switches};

pub(crate) fn gpu_cpu(sw: &Switches) -> Option<usize> {
    // The UI placement is inherited, including when perf.place is off.
    (sw.perf.gpu_cpu == GpuCpu::Hw).then(|| super::fifo_cpu(sw.cores)).flatten()
}

#[cfg(test)]
#[test]
fn only_the_hw_gpu_override_requests_a_child_pin() {
    use super::Cores;
    let cores = Cores { big: 0b11_0000, little: 0b1111 };
    let parse = |conf| Switches::parse(&|_| None, conf, "", cores);
    assert_eq!(gpu_cpu(&parse("")), None);
    let hw = parse("gpu.cpu = hw");
    assert_eq!(gpu_cpu(&hw), Some(5));
    let single = Switches { cores: Cores { big: 0b10000, little: 1 }, ..hw };
    assert_eq!(gpu_cpu(&single), None);
}
