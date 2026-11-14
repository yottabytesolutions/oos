// SPDX-License-Identifier: GPL-2.0-or-later

//! Thread placement on a big.LITTLE SoC (fork, docs/fork/perf-plan.md item 6). Without it the frame loop, the GPU
//! helper and the acquisition thread share both A72 of the RK3399 (`http::pin_to_big_cores`). With `perf.place = on`:
//! the frame loop (and the GPU helper it starts) on the first big core, the acquisition thread on the second. Workers
//! already go to the little cores (`http::to_little_cores`). Opt-in until measured on the unit; Linux only, a no-op elsewhere and on SoCs whose cores are
//! all alike. The xdma interrupt goes to a little core from the rootfs (`/proc/irq/<n>/smp_affinity`), not from here.

use std::path::Path;

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

/// (cpu, capacity) of every CPU under `root` (`/sys/devices/system/cpu`) that has a `cpu_capacity` file.
pub fn capacities(root: &Path) -> Vec<(usize, u32)> {
    (0..64)
        .filter_map(|c| {
            std::fs::read_to_string(root.join(format!("cpu{}/cpu_capacity", c)))
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .map(|v| (c, v))
        })
        .collect()
}

/// The CPUs of a big.LITTLE SoC as bit masks (bit n: CPU n), read once at start (`Switches::cores`): `little` the
/// cores below the largest `cpu_capacity`, `big` those at it. Both empty where the cores are all alike or the files
/// are missing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cores {
    pub big: u64,
    pub little: u64,
}

impl Cores {
    /// From (cpu, `cpu_capacity`) pairs.
    pub fn of(cap: &[(usize, u32)]) -> Cores {
        let Some(max) = cap.iter().map(|x| x.1).max() else { return Cores::default() };
        let mask = |big: bool| cap.iter().filter(|x| x.0 < 64 && (x.1 == max) == big).fold(0, |m, x| m | 1u64 << x.0);
        match mask(false) {
            0 => Cores::default(),
            little => Cores { big: mask(true), little },
        }
    }

    /// The cores of this machine (`/sys/devices/system/cpu`).
    pub fn read() -> Cores {
        Self::of(&capacities(Path::new("/sys/devices/system/cpu")))
    }

    /// The number of little cores: the deposit workers of the batch pipeline, one per little core.
    pub fn little_count(&self) -> Option<usize> {
        (self.little != 0).then(|| self.little.count_ones() as usize)
    }

    /// The CPUs of each role: None without two kinds of core. One big core: the frame loop and the acquisition share
    /// it, as without a plan.
    pub fn plan(&self) -> Option<Plan> {
        let big = cpus(self.big);
        let first = *big.first().filter(|_| self.little != 0)?;
        Some(Plan { ui: vec![first], hw: vec![big.get(1).copied().unwrap_or(first)] })
    }
}

/// The CPU numbers of a mask, ascending.
pub fn cpus(mask: u64) -> Vec<usize> {
    (0..64).filter(|c| mask >> c & 1 == 1).collect()
}

/// The calling thread (and the threads and processes it starts after this) onto the CPUs of `r`, when placement is on
/// (`Switches::place`: `perf.place = on` in system.conf, which also moves the xdma interrupts: rootfs
/// `oos-irq-place`; or OOS_PLACE set to any value but empty or 0).
pub fn place(r: Role, sw: &crate::sysconf::Switches) {
    if !sw.place {
        return;
    }
    #[cfg(target_os = "linux")]
    {
        let Some(plan) = sw.cores.plan() else { return };
        let cpus = plan.cpus(r);
        // SAFETY: a local CPU set for the calling thread
        let rc = unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            for &c in cpus {
                libc::CPU_SET(c, &mut set);
            }
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set)
        };
        if rc != 0 {
            eprintln!("oos-ui: placement {:?} on {:?}: {}", r, cpus, std::io::Error::last_os_error());
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = r;
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
        let p = c.plan().unwrap();
        assert_eq!((p.cpus(Role::Ui), p.cpus(Role::Hw)), (&[4][..], &[5][..]));
        assert_eq!(c.little_count(), Some(4));
        std::fs::remove_dir_all(&root).unwrap();
        // all alike, or no capacity files: nothing to place
        let root = fake_sysfs(&[(0, 1024), (1, 1024), (2, 1024), (3, 1024)]);
        assert_eq!(Cores::of(&capacities(&root)), Cores::default());
        assert_eq!(Cores::of(&capacities(&root)).little_count(), None);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(Cores::of(&[]).plan(), None);
        // one big core: the frame loop and the acquisition share it; the order of the files does not matter
        let p = Cores::of(&[(3, 1024), (2, 400), (0, 400), (1, 400)]).plan().unwrap();
        assert_eq!((p.ui.clone(), p.hw.clone()), (vec![3], vec![3]));
    }
}
