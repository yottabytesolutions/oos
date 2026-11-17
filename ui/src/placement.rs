// SPDX-License-Identifier: GPL-2.0-or-later

//! Thread placement on a big.LITTLE SoC (fork, docs/fork/perf-plan.md item 6). Without it the frame loop, the GPU
//! helper and the acquisition thread share both A72 of the RK3399 (`pin_to_big_cores`). With `perf.place = on`:
//! the frame loop (and the GPU helper it starts) on the first big core, the acquisition thread on the second. Workers
//! already go to the little cores (`to_little_cores`). Opt-in until measured on the unit; Linux only, a no-op elsewhere and on SoCs whose cores are
//! all alike. IRQ policy and procfs application live in `irq`.

pub mod irq;

#[cfg(test)]
pub use oos_hal::system::capacities;
use oos_hal::system::pin_current_thread;
pub use oos_hal::system::{cpus, Cores};

/// What a thread does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// the frame loop and the GPU helper
    Ui,
    /// the acquisition thread (`board::dho::hw`)
    Hw,
}

/// The CPUs of each role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub ui: Vec<usize>,
    pub hw: Vec<usize>,
}

impl Plan {
    pub fn cpus(&self, r: Role) -> &[usize] {
        match r {
            Role::Ui => &self.ui,
            Role::Hw => &self.hw,
        }
    }
}

/// The CPUs of each role: None without two kinds of core. One big core: the frame loop and the acquisition share
/// it, as without a plan.
#[cfg(any(target_os = "linux", test))]
fn plan(cores: Cores) -> Option<Plan> {
    let big = cpus(cores.big);
    let first = *big.first().filter(|_| cores.little != 0)?;
    Some(Plan { ui: vec![first], hw: vec![big.get(1).copied().unwrap_or(first)] })
}

/// The calling thread (and the threads and processes it starts after this) onto the CPUs of `r`, when placement is on
/// (`Switches::perf`: `perf.place = on` in system.conf, or OOS_PLACE set to any value but empty or 0).
/// Device startup also applies the explicit IRQ plan from `irq`.
pub fn place(r: Role, sw: &crate::sysconf::Switches) {
    if !sw.perf.place() {
        return;
    }
    #[cfg(target_os = "linux")]
    {
        let Some(plan) = plan(sw.cores) else { return };
        let cpus = plan.cpus(r);
        if let Err(error) = pin_current_thread(cpus) {
            eprintln!("oos-ui: placement {:?} on {:?}: {}", r, cpus, error);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = r;
}

/// Web workers run at SCHED_IDLE, with nice 10 as fallback, on the little cores.
pub fn lower_priority(little: u64) {
    // Test workers must keep normal priority so busy parallel tests can make progress.
    if !cfg!(test) {
        let _ = oos_hal::system::idle_current_thread(&cpus(little));
    }
}

/// The slower cores, empty where all cores are alike.
pub fn little_core_list() -> Vec<usize> {
    cpus(Cores::read().little)
}

/// The calling thread onto the slower cores at normal priority.
pub fn to_little_cores(little: u64) {
    let _ = pin_current_thread(&cpus(little));
}

/// The calling thread and its future children onto the fastest cores.
#[cfg(target_os = "linux")]
pub fn pin_to_big_cores(cores: Cores) {
    if cores.little != 0 {
        let _ = pin_current_thread(&cpus(cores.big));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_sysfs(cap: &[(usize, u32)]) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oos-place-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        for &(c, v) in cap {
            let d = root.join(format!("cpu{}", c));
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("cpu_capacity"), format!("{}\n", v)).unwrap();
        }
        // a directory without the file (cpufreq, cpuidle) is no CPU
        std::fs::create_dir_all(root.join("cpufreq")).unwrap();
        root
    }

    // Break it catches: the acquisition thread on the frame loop's core, a plan on a SoC whose cores are all alike,
    // big and little swapped.
    #[test]
    fn rk3399_frame_loop_and_acquisition_get_one_a72_each() {
        let root = fake_sysfs(&[(0, 485), (1, 485), (2, 485), (3, 485), (4, 1024), (5, 1024)]);
        let c = Cores::of(&capacities(&root));
        assert_eq!(c, Cores { big: 0b11_0000, little: 0b1111 });
        let p = plan(c).unwrap();
        assert_eq!((p.cpus(Role::Ui), p.cpus(Role::Hw)), (&[4][..], &[5][..]));
        assert_eq!(c.little_count(), Some(4));
        std::fs::remove_dir_all(&root).unwrap();
        // all alike, or no capacity files: nothing to place
        let root = fake_sysfs(&[(0, 1024), (1, 1024), (2, 1024), (3, 1024)]);
        assert_eq!(Cores::of(&capacities(&root)), Cores::default());
        assert_eq!(Cores::of(&capacities(&root)).little_count(), None);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(plan(Cores::of(&[])), None);
        // one big core: the frame loop and the acquisition share it; the order of the files does not matter
        let p = plan(Cores::of(&[(3, 1024), (2, 400), (0, 400), (1, 400)])).unwrap();
        assert_eq!((p.ui.clone(), p.hw.clone()), (vec![3], vec![3]));
    }
}
