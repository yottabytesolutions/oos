// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::mdns::Service;

mod events;
mod wire;

fn config() -> Config {
    Config {
        on: true,
        name: "oos-test".into(),
        instance: "OOS test".into(),
        services: [
            ("_http._tcp", 8080, vec![("path".into(), Some("/".into()))]),
            ("_scpi-raw._tcp", 5555, vec![]),
            ("_vxi-11._tcp", 111, vec![]),
            ("_ssh._tcp", 22, vec![]),
            ("_lxi._tcp", 8080, vec![("path".into(), Some("/lxi/identification".into()))]),
            ("_hislip._tcp", 4880, vec![]),
        ]
        .into_iter()
        .map(|(kind, port, txt)| Service { kind: kind.into(), port, txt })
        .collect(),
        v6: true,
    }
}

#[test]
fn six_services_keep_their_names_ports_and_txt_and_track_addresses() {
    let cfg = config();
    for service in &cfg.services {
        let info = registration(&cfg, service).unwrap();
        assert_eq!(info.get_type(), format!("{}.local.", service.kind));
        assert_eq!(info.get_fullname(), format!("OOS test.{}.local.", service.kind));
        assert_eq!(info.get_hostname(), "oos-test.local.");
        assert_eq!(info.get_port(), service.port);
        assert!(info.requires_probe(), "the crate resolves host and instance conflicts");
        assert!(info.is_addr_auto(), "DHCP, link-local and SLAAC changes follow the daemon's interfaces");
        let txt: Vec<_> = info
            .get_properties()
            .iter()
            .map(|property| (property.key().to_string(), property.val().map(|_| property.val_str().to_string())))
            .collect();
        assert_eq!(txt, service.txt);
    }
}

#[tokio::test]
async fn daemon_refreshes_interfaces_and_withdraws_before_shutdown() {
    let mut cfg = config();
    for v6 in [false, true] {
        let daemon = ServiceDaemon::new_with_port(0).unwrap();
        cfg.v6 = v6;
        configure(&daemon, &cfg, Some(&[])).unwrap();
        assert!(joined_addresses(&daemon).await.unwrap().is_empty());
        assert_eq!(daemon.get_ip_check_interval().unwrap(), 2);
        for service in &cfg.services {
            let info = registration(&cfg, service).unwrap();
            let removed = daemon_reply(daemon.unregister(info.get_fullname())).await.unwrap();
            assert!(matches!(removed, mdns_sd::UnregisterStatus::OK));
            daemon.register(info).unwrap();
        }
        assert_eq!(daemon_reply(daemon.shutdown()).await.unwrap(), DaemonStatus::Shutdown);
    }
}

#[test]
fn maximum_labels_reserve_space_for_conflict_counters() {
    let mut cfg = config();
    cfg.name = "x".repeat(crate::mdns::NAME_MAX);
    cfg.instance = crate::mdns::instance_name("DHO804", None, &cfg.name);
    assert!(crate::mdns::name_ok(&cfg.name));
    let info = registration(&cfg, &cfg.services[0]).unwrap();
    let host = host_label(info.get_hostname());
    let instance = info.get_fullname().strip_suffix("._http._tcp.local.").unwrap();
    assert!(cfg.name.starts_with(host));
    assert!(cfg.instance.starts_with(instance));
    for counter in [2, 10, 100, u32::MAX] {
        assert!(crate::mdns::name_ok(&format!("{host}-{counter}")));
        assert!(format!("{instance} ({counter})").len() <= crate::mdns::NAME_MAX);
    }
    assert_eq!(cfg.name.len(), crate::mdns::NAME_MAX, "registration preserves the setting");
}

#[test]
fn shortened_host_labels_do_not_end_with_a_hyphen() {
    let mut cfg = config();
    cfg.name = format!("{}-{}", "x".repeat(51), "y".repeat(11));
    let info = registration(&cfg, &cfg.services[0]).unwrap();
    let host = host_label(info.get_hostname());
    assert!(crate::mdns::name_ok(host));
    assert!(crate::mdns::name_ok(&format!("{host}-{}", u32::MAX)));
}

#[test]
fn shortened_instance_labels_preserve_utf8() {
    let mut cfg = config();
    cfg.instance = format!("{}é{}", "x".repeat(49), "y".repeat(12));
    let info = registration(&cfg, &cfg.services[0]).unwrap();
    let instance = info.get_fullname().strip_suffix("._http._tcp.local.").unwrap();
    assert_eq!(instance, "x".repeat(49));
    assert!(format!("{instance} ({})", u32::MAX).len() <= crate::mdns::NAME_MAX);
}

#[test]
fn shutdown_cancels_idle_discovery_and_keeps_the_shared_executor_alive() {
    let runtime = Runtime::new().unwrap();
    let mut mdns = Mdns::new(runtime.clone(), Some(Vec::new()));
    mdns.set(Config { on: false, ..config() });
    assert!(mdns.control.is_none());
    mdns.set(Config { services: Vec::new(), ..config() });
    mdns.control.take();
    mdns.stopped.take().unwrap().recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
    let (send, receive) = mpsc::channel();
    runtime.spawn(async move {
        send.send(()).unwrap();
    });
    receive.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
    mdns.shutdown(Instant::now());
}

#[tokio::test]
async fn switch_changes_and_cancellation_stop_an_active_registration() {
    let (control, mut changed) = watch::channel(config());
    let cfg = changed.borrow_and_update().clone();
    control.send_replace(Config { on: false, ..cfg.clone() });
    let daemon = ServiceDaemon::new_with_port(0).unwrap();
    assert!(advertise(&daemon, &cfg, Some(&[]), &mut changed).await.unwrap());
    assert_eq!(daemon_reply(daemon.shutdown()).await.unwrap(), DaemonStatus::Shutdown);
    drop(control);
    let daemon = ServiceDaemon::new_with_port(0).unwrap();
    assert!(!advertise(&daemon, &cfg, Some(&[]), &mut changed).await.unwrap());
    assert_eq!(daemon_reply(daemon.shutdown()).await.unwrap(), DaemonStatus::Shutdown);
}

fn resolved(browser: &ServiceDaemon, cfg: &Config, service: &Service) -> mdns_sd::Receiver<mdns_sd::ServiceEvent> {
    let events = browser.browse(&format!("{}.local.", service.kind)).unwrap();
    loop {
        let event = events.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
        if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
            if info.get_hostname() != format!("{}.local.", cfg.name) {
                continue;
            }
            assert_eq!(info.get_port(), service.port);
            assert!(!info.get_addresses().is_empty());
            assert_eq!(info.get_properties(), registration(cfg, service).unwrap().get_properties());
            return events;
        }
    }
}

#[test]
#[ignore = "live multicast: set OOS_MDNS_IF"]
fn host_responder() {
    let only: Vec<String> =
        std::env::var("OOS_MDNS_IF").expect("set OOS_MDNS_IF").split(',').map(str::to_string).collect();
    let mut cfg = config();
    cfg.name = format!("oos-test-{}", std::process::id());
    cfg.instance = format!("OOS test ({})", std::process::id());
    let mut mdns = Mdns::new(Runtime::new().unwrap(), Some(only.clone()));
    mdns.set(cfg.clone());
    let browser = ServiceDaemon::new().unwrap();
    browser.disable_interface(IfKind::All).unwrap();
    for name in &only {
        browser.enable_interface(name.as_str()).unwrap();
    }
    for service in cfg.services.clone() {
        let events = resolved(&browser, &cfg, &service);
        let name = registration(&cfg, &service).unwrap().get_fullname().to_string();
        cfg.services.retain(|s| s.kind != service.kind);
        mdns.set(cfg.clone());
        loop {
            let event = events.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
            if let mdns_sd::ServiceEvent::ServiceRemoved(_, removed) = event {
                if removed == name {
                    break;
                }
            }
        }
        browser.stop_browse(&format!("{}.local.", service.kind)).unwrap();
    }
    cfg.services = config().services;
    mdns.set(cfg.clone());
    let service = &cfg.services[0];
    let events = resolved(&browser, &cfg, service);
    cfg.on = false;
    mdns.set(cfg);
    loop {
        if let mdns_sd::ServiceEvent::ServiceRemoved(_, _) =
            events.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap()
        {
            break;
        }
    }
    mdns.shutdown(Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD);
    browser.shutdown().unwrap().recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
}
