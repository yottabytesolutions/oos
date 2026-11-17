// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

/// Parameters of xdma.ko without `xdma.params`: DMA completion by polling the write-back word, what OOS was verified
/// with. The stock system loads it with interrupts.
const XDMA_DEFAULT: &str = "poll_mode=1";

/// The xdma.ko parameters of `xdma.params` in the system settings (`oos-conf`; None or empty = `XDMA_DEFAULT`). Only
/// `poll_mode=0|1` and `interrupt_mode=0..3` (0 auto, 1 MSI, 2 legacy INTx, 3 MSI-X), each at most once: the line goes
/// to `insmod` through a shell.
fn xdma_params(v: Option<&str>) -> Result<String, String> {
    let v = v.map(str::trim).unwrap_or("");
    if v.is_empty() {
        return Ok(XDMA_DEFAULT.into());
    }
    let mut seen = Vec::new();
    for w in v.split_whitespace() {
        let ok = matches!(
            w,
            "poll_mode=0"
                | "poll_mode=1"
                | "interrupt_mode=0"
                | "interrupt_mode=1"
                | "interrupt_mode=2"
                | "interrupt_mode=3"
        );
        let key = w.split('=').next().unwrap_or(w);
        if !ok || seen.contains(&key) {
            return Err(format!("xdma.params: {:?} is not allowed (poll_mode=0|1, interrupt_mode=0..3, each once)", w));
        }
        seen.push(key);
    }
    Ok(v.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// The xdma.ko parameters for this start: `xdma_params` of the system setting, except in a start by kexec (`kexec`:
/// `oos.kexec=` on the command line), which always polls. The GICv3 keeps its ITS tables across kexec (docs/update.md),
/// the MSI completions of the C2H engine never arrive there and every read timed out with `poll_mode=0`. Returns the
/// line and, when the setting was overridden or refused, why.
fn xdma_params_for(conf: Option<&str>, kexec: bool) -> (String, Option<String>) {
    let set = xdma_params(conf);
    if kexec {
        let why = match set {
            Ok(v) if v == XDMA_DEFAULT => None,
            Ok(v) => Some(format!("started by kexec: MSI does not reach this kernel, xdma.params {:?} overridden", v)),
            Err(e) => Some(e),
        };
        return (XDMA_DEFAULT.into(), why);
    }
    match set {
        Ok(v) => (v, None),
        Err(e) => (XDMA_DEFAULT.into(), Some(e)),
    }
}

/// PCIe host + xdma bring-up with retries (the loop of oos-fpga-up, without the FPGA boot), then BAR1. The device and
/// the xdma nodes are polled every 20 ms instead of the fixed 2 s / 1 s sleeps of oos-fpga-up (`insmod` returns once
/// the driver probed, so a link that is up shows at once).
#[cfg(target_os = "linux")]
pub(crate) fn up() -> std::io::Result<oos_hal::dho::bus::Bar1> {
    let sh = |c: &str| std::process::Command::new("sh").arg("-c").arg(c).status().map(|s| s.success()).unwrap_or(false);
    let poll = |cond: &dyn Fn() -> bool, ms: u64| {
        let t = std::time::Instant::now();
        while !cond() {
            if t.elapsed().as_millis() as u64 >= ms {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        true
    };
    let dev = || std::path::Path::new("/sys/bus/pci/devices/0000:01:00.0").exists();
    let m = "/lib/modules/$(uname -r)";
    if !sh("lsmod | grep -q '^xdma '") {
        for _ in 0..6 {
            sh(&format!("insmod {}/kernel/drivers/pci/controller/pcierockchiphost.ko 2>/dev/null", m));
            if poll(&dev, 2000) {
                break;
            }
            sh("rmmod pcierockchiphost 2>/dev/null");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        if !dev() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "FPGA did not appear on PCIe"));
        }
        let conf = std::process::Command::new("/bin/oos-conf")
            .arg("xdma.params")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
        let kexec = std::fs::read_to_string("/proc/cmdline")
            .is_ok_and(|c| c.split_whitespace().any(|w| w.starts_with(oos_hal::update::kexec::KEY)));
        let (params, why) = xdma_params_for(conf.as_deref(), kexec);
        if let Some(w) = why {
            eprintln!("oos-fpgad: {}; loading xdma with {}", w, params);
        }
        eprintln!("oos-fpgad: insmod xdma.ko {}", params);
        if !sh(&format!("insmod {}/updates/xdma.ko {}", m, params)) {
            return Err(std::io::Error::other("insmod xdma failed"));
        }
        let nodes = || ["/dev/xdma0_bypass", "/dev/xdma0_c2h_0"].iter().all(|n| std::path::Path::new(n).exists());
        if !poll(&nodes, 3000) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "xdma loaded, but /dev/xdma0_* did not appear",
            ));
        }
    }
    oos_hal::dho::bus::Bar1::open()
}

/// Unloads xdma and the PCIe host driver, the step before the BOOT command is sent again (`io::power_on`): the FPGA
/// must not reconfigure behind a bound endpoint. An error, with what `rmmod` said, if either module is still loaded
/// afterwards.
#[cfg(target_os = "linux")]
pub(crate) fn down() -> std::io::Result<()> {
    let run = |c: &str| std::process::Command::new("sh").arg("-c").arg(c).output().ok();
    let said: Vec<String> = ["rmmod xdma 2>&1", "rmmod pcierockchiphost 2>&1"]
        .iter()
        .filter_map(|c| run(c))
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if run("lsmod | grep -qE '^(xdma|pcierockchiphost) '").map(|o| o.status.success()).unwrap_or(false) {
        return Err(std::io::Error::other(format!(
            "xdma or the PCIe host driver is still loaded after rmmod ({})",
            said.join("; ")
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdma_parameters_from_the_system_settings() {
        assert_eq!(xdma_params(None).unwrap(), "poll_mode=1");
        assert_eq!(xdma_params(Some(" \n")).unwrap(), "poll_mode=1");
        assert_eq!(xdma_params(Some("poll_mode=0\n")).unwrap(), "poll_mode=0");
        assert_eq!(xdma_params(Some("poll_mode=0   interrupt_mode=2")).unwrap(), "poll_mode=0 interrupt_mode=2");
        for bad in [
            "poll_mode=2",
            "poll_mode=0; reboot",
            "poll_mode=0 poll_mode=1",
            "interrupt_mode=4",
            "c2h_timeout=1",
            "poll_mode=0 $(id)",
        ] {
            assert!(xdma_params(Some(bad)).is_err(), "{}", bad);
        }
    }

    #[test]
    fn a_kexec_start_polls_whatever_the_setting() {
        assert_eq!(xdma_params_for(Some("poll_mode=0"), false), ("poll_mode=0".to_string(), None));
        let (v, why) = xdma_params_for(Some("poll_mode=0"), true);
        assert_eq!(v, "poll_mode=1");
        assert!(why.is_some_and(|w| w.contains("kexec") && w.contains("poll_mode=0")));
        assert_eq!(xdma_params_for(Some("poll_mode=0 interrupt_mode=1"), true).0, "poll_mode=1");
        // the default needs no note; a refused setting says why in both starts
        assert_eq!(xdma_params_for(None, true), ("poll_mode=1".to_string(), None));
        assert!(
            xdma_params_for(Some("poll_mode=7"), true).1.is_some()
                && xdma_params_for(Some("poll_mode=7"), false).1.is_some()
        );
    }
}
