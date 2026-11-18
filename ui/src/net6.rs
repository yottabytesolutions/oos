// Copyright (C) 2026 OOS contributors
// SPDX-License-Identifier: GPL-2.0-or-later

//! IPv6 for the services (fork, docs/network.md 8): with `net.ipv6 = on` the web remote, SCPI and VXI-11 listen on
//! `[::]` (both families: rcS sets `bindv6only = 0`, IPv4 peers arrive as `::ffff:a.b.c.d`, which `netacl` maps back)
//! and mDNS publishes the IPv6 addresses. Off (the default of other images): IPv4 only, as upstream. The addresses
//! themselves are the kernel's SLAAC addresses (`oos-net6`); `onlink6` in `net.allow` admits the prefixes on the link,
//! which `oos-firewall watch` keeps in `PREFIX_FILE`.

use core::net::Ipv6Addr;

/// The IPv6 prefixes on the link, one `addr/len` per line (`oos-firewall`).
pub const PREFIX_FILE: &str = "/tmp/oos-net6.prefixes";
/// Shortest prefix taken from the link: a router advertisement with a shorter on-link prefix (a rogue one with
/// `2000::/3`) must not open the scope to the internet. The same bound is in `oos-firewall`.
pub const PREFIX_MIN: u8 = 48;

/// `env` if set, else the `net.ipv6` line of `conf`; off unless `on`.
pub fn enabled_from(env: Option<&str>, conf: &str) -> bool {
    crate::drawpipe::conf_value(env, conf, "net.ipv6").as_deref() == Some("on")
}

/// Device service ports retain their family policy; host harnesses always use IPv4 loopback and port 0.
#[derive(Clone, Copy, Debug)]
pub enum Port {
    Service(u16),
    Host,
}

impl From<u16> for Port {
    fn from(port: u16) -> Self {
        Self::Service(port)
    }
}

/// Host startup errors must reach the launcher through stderr and a failing exit status.
pub fn host_or_exit<T>(result: std::io::Result<T>) -> T {
    match result {
        Ok(value) => value,
        Err(e) => {
            eprintln!("oos-ui: host startup failed: {e}");
            std::process::exit(1);
        }
    }
}

/// What a person who started `oos-ui --serve` by hand reads: where to point the browser, the SCPI and HiSLIP clients
/// and a VXI-11 client (its port mapper is not on 111, so the client needs the ports).
fn serve_banner(web: u16, scpi: u16, hislip: Option<u16>, (tcp, udp, core): (u16, u16, u16)) -> String {
    let hislip = hislip.map_or_else(|| "HiSLIP unavailable".to_string(), |p| format!("HiSLIP on 127.0.0.1:{p}"));
    format!(
        "oos-ui: open http://127.0.0.1:{web}/ in a browser; SCPI on 127.0.0.1:{scpi}; {hislip}; \
         VXI-11 mapper TCP {tcp} UDP {udp}, core {core}"
    )
}

/// Publish only after every host listener is bound. The launcher owns the private run directory. Without a launcher
/// (`OOS_HARNESS_BUILD` unset) a person started the server: it prints where to connect instead.
pub fn host_ready(
    web: Option<u16>,
    scpi: Option<u16>,
    vxi: Option<(u16, u16, u16)>,
    hislip: Option<u16>,
) -> std::io::Result<()> {
    let (Some(web), Some(scpi), Some((tcp, udp, core))) = (web, scpi, vxi) else {
        return Err(std::io::Error::other("a host listener is unavailable; readiness was not published"));
    };
    let Some(build) = std::env::var_os("OOS_HARNESS_BUILD") else {
        println!("{}", serve_banner(web, scpi, hislip, (tcp, udp, core)));
        return Ok(());
    };
    let build = build.into_string().unwrap_or_default();
    if build.is_empty() || !build.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "OOS_HARNESS_BUILD must be non-empty hexadecimal; launch with tools/host_harness.py",
        ));
    }
    let record = format!(
        "{{\"pid\":{},\"build\":\"{}\",\"web\":{},\"scpi\":{},\"vxi_tcp\":{},\"vxi_udp\":{},\"vxi_core\":{}}}\n",
        std::process::id(),
        build,
        web,
        scpi,
        tcp,
        udp,
        core
    );
    let Some(path) = std::env::var_os("OOS_HARNESS_READY") else {
        println!("{}", record.trim());
        return Ok(());
    };
    let path = std::path::PathBuf::from(path);
    let pending = path.with_extension("pending");
    std::fs::write(&pending, record).and_then(|()| std::fs::rename(pending, path))
}

/// The prefixes of a `PREFIX_FILE` text; what does not parse, is IPv4 or shorter than `PREFIX_MIN` is left out.
pub fn parse_prefixes(text: &str) -> Vec<(Ipv6Addr, u8)> {
    text.split_whitespace()
        .filter_map(|w| {
            let (a, n) = w.split_once('/')?;
            let n = n.parse::<u8>().ok().filter(|n| (PREFIX_MIN..=128).contains(n))?;
            Some((a.parse::<Ipv6Addr>().ok()?, n))
        })
        .collect()
}

/// The prefixes on the link now (none without the file).
pub fn onlink() -> Vec<(Ipv6Addr, u8)> {
    std::fs::read_to_string(PREFIX_FILE).map(|t| parse_prefixes(&t)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_startup_failure_reports_cause_and_fails() {
        if std::env::var_os("OOS_TEST_STARTUP_CHILD").is_some() {
            host_or_exit::<()>(Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test startup cause")));
            return;
        }
        assert_eq!(host_or_exit(Ok(7)), 7);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "net6::tests::host_startup_failure_reports_cause_and_fails", "--nocapture"])
            .env("OOS_TEST_STARTUP_CHILD", "1")
            .output()
            .unwrap();
        assert_eq!(child.status.code(), Some(1));
        let stderr = String::from_utf8(child.stderr).unwrap();
        assert!(stderr.contains("host startup failed"));
        assert!(stderr.contains("test startup cause"));
    }

    #[test]
    fn host_ready_record() {
        // Give each child its own environment; parallel tests never mutate process-wide variables.
        if std::env::var_os("OOS_TEST_READY_CHILD").is_some() {
            let build = std::env::var("OOS_HARNESS_BUILD").unwrap();
            let result = host_ready(Some(1001), Some(1002), Some((1003, 1004, 1005)), Some(1006));
            assert_eq!(result.is_ok(), !build.is_empty() && build.bytes().all(|b| b.is_ascii_hexdigit()));
            return;
        }
        let run = std::env::temp_dir().join(format!("oos-ready-{}", std::process::id()));
        std::fs::create_dir(&run).unwrap();
        let path = run.join("ready.json");
        for build in ["0123456789abcdefABCDEF", "not-hex", ""] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "net6::tests::host_ready_record", "--nocapture"])
                .env("OOS_TEST_READY_CHILD", "1")
                .env("OOS_HARNESS_BUILD", build)
                .env("OOS_HARNESS_READY", &path)
                .spawn()
                .unwrap();
            let pid = child.id();
            assert!(child.wait().unwrap().success());
            if build == "0123456789abcdefABCDEF" {
                assert_eq!(
                    std::fs::read_to_string(&path).unwrap(),
                    format!(
                        "{{\"pid\":{pid},\"build\":\"{build}\",\"web\":1001,\"scpi\":1002,\"vxi_tcp\":1003,\"vxi_udp\":1004,\"vxi_core\":1005}}\n"
                    )
                );
                std::fs::remove_file(&path).unwrap();
            } else {
                assert!(!path.exists(), "invalid builds must not publish readiness");
            }
            assert!(!path.with_extension("pending").exists());
        }
        std::fs::remove_dir(run).unwrap();
    }

    #[test]
    fn serve_banner_names_every_port_a_client_needs() {
        assert_eq!(
            serve_banner(40001, 40002, Some(40006), (40003, 40004, 40005)),
            "oos-ui: open http://127.0.0.1:40001/ in a browser; SCPI on 127.0.0.1:40002; HiSLIP on 127.0.0.1:40006; \
             VXI-11 mapper TCP 40003 UDP 40004, core 40005"
        );
        assert!(serve_banner(40001, 40002, None, (40003, 40004, 40005)).contains("HiSLIP unavailable"));
    }

    #[test]
    fn switch_and_prefixes() {
        assert!(!enabled_from(None, ""), "off by default");
        assert!(enabled_from(None, "net.ipv6 = on\n") && !enabled_from(Some("off"), "net.ipv6 = on\n"));
        let p = parse_prefixes("2001:db8:1:2::/64\nfd00:1::/56 10.0.0.0/8 2000::/3 x/64 2001:db8::/129 ::1/128\n");
        let want: Vec<(Ipv6Addr, u8)> = vec![
            ("2001:db8:1:2::".parse().unwrap(), 64),
            ("fd00:1::".parse().unwrap(), 56),
            ("::1".parse().unwrap(), 128),
        ];
        assert_eq!(p, want);
    }
}
