// SPDX-License-Identifier: GPL-2.0-or-later
//! Startup IRQ policy. The tuning evidence leaves display, GPU and storage IRQs unchanged.
use super::{cpus, Cores};
use std::io;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub struct Assignment {
    irq: u32,
    cpu: usize,
}

/// Keep XDMA and Ethernet off the busiest A53 and on different cores when possible.
fn plan(cores: Cores, busiest: usize, interrupts: &str) -> Vec<Assignment> {
    let available: Vec<_> = cpus(cores.little).into_iter().filter(|c| *c != busiest).collect();
    let dma = available.last().copied();
    let ethernet = dma
        .filter(|_| cores.little & 1 != 0)
        .map(|cpu| available.iter().rev().copied().find(|&c| c != cpu && c != 0).unwrap_or(cpu));
    interrupts
        .lines()
        .filter_map(|line| {
            let (number, description) = line.split_once(':')?;
            let irq = number.trim().parse().ok()?;
            let words: Vec<_> = description.split_whitespace().collect();
            let cpu = if description.contains("MSI") && words.iter().any(|w| w.starts_with("xdma")) {
                dma?
            } else if words.iter().any(|w| *w == "eth0" || w.starts_with("eth0-")) {
                ethernet?
            } else {
                return None;
            };
            Some(Assignment { irq, cpu })
        })
        .collect()
}

/// `root` is procfs, or a fake procfs tree. Apply only discovered IRQs and verify every write.
pub fn apply(root: &Path, plan: &[Assignment]) -> Vec<io::Error> {
    if plan.is_empty() {
        eprintln!("oos-ui: irq_placement empty_plan");
    }
    plan.iter()
        .filter_map(|entry| {
            let path = root.join(format!("irq/{}/smp_affinity_list", entry.irq));
            let cpu = entry.cpu.to_string();
            let result = std::fs::write(&path, &cpu).and_then(|()| {
                if std::fs::read_to_string(&path)?.trim() == cpu {
                    Ok(())
                } else {
                    Err(io::Error::other("IRQ affinity read-back mismatch"))
                }
            });
            if result.is_ok() {
                eprintln!("oos-ui: irq_placement irq={} cpu={}", entry.irq, entry.cpu);
            }
            result.err()
        })
        .collect()
}

pub fn startup(sw: &crate::sysconf::Switches) {
    if !sw.perf.place() {
        return;
    }
    // CPU0 is the crowded core in the supplied baseline. Unit acceptance must
    // confirm this conservative choice against per-thread deposit measurements.
    let root = Path::new("/proc");
    let result = std::fs::read_to_string(root.join("interrupts"));
    match result {
        Ok(interrupts) => {
            for error in apply(root, &startup_plan(sw, &interrupts)) {
                eprintln!("oos-ui: irq_placement error={}", error);
            }
        }
        Err(error) => eprintln!("oos-ui: irq_placement error={}", error),
    }
}

fn startup_plan(sw: &crate::sysconf::Switches, interrupts: &str) -> Vec<Assignment> {
    plan(sw.cores, 0, interrupts)
}

#[cfg(test)]
mod tests;
