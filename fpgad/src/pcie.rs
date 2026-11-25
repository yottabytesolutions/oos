// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

#[derive(Default)]
struct XdmaParams<'a> {
    poll: Option<&'a str>,
    interrupt: Option<&'a str>,
    c2h: Option<&'a str>,
    h2c: Option<&'a str>,
}

impl<'a> XdmaParams<'a> {
    // Keep the whitelist in step with oos-fpga-up. Only these literal arguments reach insmod.
    fn parse(conf: &'a str) -> Result<Self, String> {
        let mut params = Self::default();
        for token in conf.split_whitespace() {
            let slot = match token.split_once('=') {
                Some(("poll_mode", "0" | "1")) => &mut params.poll,
                Some(("interrupt_mode", "0" | "1" | "2" | "3")) => &mut params.interrupt,
                Some(("c2h_timeout", value)) if timeout_value(value) => &mut params.c2h,
                Some(("h2c_timeout", value)) if timeout_value(value) => &mut params.h2c,
                _ => return Err(format!("xdma.params: {token:?} is not allowed")),
            };
            if slot.replace(token).is_some() {
                return Err(format!("xdma.params: {token:?} repeats a parameter"));
            }
        }
        Ok(params)
    }

    fn arguments(&self, kexec: bool) -> String {
        let (poll, interrupt) = if kexec || (self.poll.is_none() && self.interrupt.is_none()) {
            (Some("poll_mode=1"), None)
        } else {
            (self.poll, self.interrupt)
        };
        [poll, interrupt, Some(self.c2h.unwrap_or("c2h_timeout=1")), self.h2c]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn timeout_value(value: &str) -> bool {
    matches!(value.as_bytes(), [b'1'..=b'9'] | b"10")
}

/// Kexec forces polling because the GICv3 retains the old kernel's ITS tables (docs/update.md).
/// Valid timeout overrides apply in both boot paths.
fn xdma_params_for(conf: Option<&str>, kexec: bool) -> (String, Option<String>) {
    match XdmaParams::parse(conf.unwrap_or_default()) {
        Ok(params) => {
            let why = (kexec && (params.poll == Some("poll_mode=0") || params.interrupt.is_some()))
                .then(|| "started by kexec: MSI does not reach this kernel, forcing poll_mode=1".into());
            (params.arguments(kexec), why)
        }
        Err(e) => (XdmaParams::default().arguments(kexec), Some(e)),
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

    const DEFAULT: &str = "poll_mode=1 c2h_timeout=1";

    fn check(conf: &str, kexec: bool, expected: &str, warning: bool) {
        let (args, why) = xdma_params_for(Some(conf), kexec);
        assert_eq!(args, expected, "{conf:?}");
        assert_eq!(why.is_some(), warning, "{conf:?}");

        let script = include_str!("../../rootfs/board/dho/bin/oos-fpga-up");
        let (helper, _) = script.split_once("\nM=").unwrap();
        let command = format!("{helper}\nxdma_params \"$1\" \"$2\"");
        let cmdline = if kexec { "quiet oos.kexec=1 rw" } else { "quiet rw" };
        let output =
            std::process::Command::new("sh").args(["-c", &command, "xdma_params", conf, cmdline]).output().unwrap();
        assert!(output.status.success(), "{conf:?}");
        assert_eq!(std::str::from_utf8(&output.stdout).unwrap().trim_end(), expected, "{conf:?}");
        assert_eq!(!output.stderr.is_empty(), warning, "{conf:?}");
    }

    #[test]
    fn xdma_parameters_from_the_system_settings() {
        assert_eq!(xdma_params_for(None, false), (DEFAULT.to_owned(), None));
        check(" \n", false, DEFAULT, false);
        check("poll_mode=0\n", false, "poll_mode=0 c2h_timeout=1", false);
        check("h2c_timeout=10 c2h_timeout=2", false, "poll_mode=1 c2h_timeout=2 h2c_timeout=10", false);
        check("interrupt_mode=2", false, "interrupt_mode=2 c2h_timeout=1", false);
        check(
            "poll_mode=0\tinterrupt_mode=3 c2h_timeout=10",
            false,
            "poll_mode=0 interrupt_mode=3 c2h_timeout=10",
            false,
        );
        for bad in [
            "poll_mode=2",
            "poll_mode=0; reboot",
            "poll_mode=0 poll_mode=1",
            "interrupt_mode=4",
            "poll_mode=0 $(id)",
            "c2h_timeout=0",
            "c2h_timeout=-1",
            "c2h_timeout=11",
            "c2h_timeout=10.0",
            "c2h_timeout=01",
            "c2h_timeout=1 c2h_timeout=2",
            "h2c_timeout=3 h2c_timeout=4",
            "h2c_timeout=1*",
            "h2c_timeout=1\nc2h_timeout=100",
        ] {
            check(bad, false, DEFAULT, true);
        }
    }

    #[test]
    fn kexec_forces_polling_and_keeps_timeout_overrides() {
        check("", true, DEFAULT, false);
        check("poll_mode=7 c2h_timeout=9", true, DEFAULT, true);
        check("c2h_timeout=9", true, "poll_mode=1 c2h_timeout=9", false);
        check(
            "poll_mode=0 interrupt_mode=1 c2h_timeout=10 h2c_timeout=3",
            true,
            "poll_mode=1 c2h_timeout=10 h2c_timeout=3",
            true,
        );
    }
}
