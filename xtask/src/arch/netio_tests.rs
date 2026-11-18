// SPDX-License-Identifier: GPL-2.0-or-later
//! Exercise the actual M15a boundary rules, including forbidden dependency imports.
use super::{modules, rules};
use crate::{repo, sources::Source};

fn source(module: &str, text: &str) -> Source {
    Source {
        rel: format!("ui/src/{}.rs", module.replace("::", "/")),
        module: module.into(),
        test: false,
        file: syn::parse_file(text).unwrap(),
        text: text.into(),
    }
}

#[test]
fn runtime_is_confined_and_io_cannot_import_application_state() {
    let actual = super::load(&repo::root()).unwrap();
    let confines: Vec<_> = actual.confines.iter().filter(|r| r.krate == "ui" && r.path == "tokio").collect();
    let layers: Vec<_> = actual.layers.iter().filter(|r| r.krate == "ui" && r.module == "netio").collect();
    assert_eq!((confines.len(), layers.len()), (1, 1));
    let check =
        modules::CrateRules { confines, layers, deps: [("tokio".into(), "tokio".into())].into(), ..Default::default() };
    for module in ["scpi", "system", "ui"] {
        assert!(!modules::check(&[source(module, "use tokio::runtime::Builder;")], &check).is_empty());
    }
    for path in ["ui::Ui", "scpi::Scpi", "model::Model", "ui::change::intake"] {
        assert!(!modules::check(&[source("netio", &format!("use crate::{path};"))], &check).is_empty());
    }
    assert!(modules::check(
        &[source("netio", "use tokio::net::TcpStream; use crate::netio::port::RemoteRequest;")],
        &check
    )
    .is_empty());
    let _: rules::Rules = actual;
}

#[test]
fn standard_network_io_stays_inside_the_recorded_adapters() {
    let actual = super::load(&repo::root()).unwrap();
    let confines: Vec<_> = actual.confines.iter().filter(|r| r.krate == "ui" && r.path == "std::net").collect();
    assert_eq!(confines.len(), 1);
    let check = modules::CrateRules { confines, ..Default::default() };
    for text in [
        "use std::net::TcpStream;",
        "use std as s; fn f() { s::net::TcpStream::connect(()); }",
        "extern crate std as s; fn f() { s::net::UdpSocket::bind(()); }",
        "use std::{net as n}; fn f() { n::TcpListener::bind(()); }",
        "use std::net::*; fn f() { TcpStream::connect(()); }",
        "fn f() { use std as s; s::net::UdpSocket::bind(()); }",
        "macro_rules! socket { () => { std::net::TcpStream } }",
    ] {
        assert!(!modules::check(&[source("ui", text)], &check).is_empty(), "{text}");
    }
    let sources = [
        source("", "mod netio; mod ui;"),
        source("netio", "pub use std::net as sockets;"),
        source("ui", "fn f() { crate::netio::sockets::TcpStream::connect(()); }"),
    ];
    assert_eq!(modules::check(&sources, &check).len(), 1);
    for (module, text) in [
        ("netio", "use std::net::UdpSocket;"),
        ("sntp", "use std::net::UdpSocket;"),
        ("ui::bode", "use std::net::TcpStream;"),
        ("gpu", "use std::net::Shutdown;"),
        ("ui", "use core::net::{Ipv4Addr, SocketAddr};"),
        ("scpi", "#[cfg(test)] use std::net::TcpStream;"),
        ("ui", "#[cfg(test)] mod tests { use std::net::TcpListener; }"),
    ] {
        assert!(modules::check(&[source(module, text)], &check).is_empty(), "{module}: {text}");
    }
}

#[test]
fn port_helpers_keep_only_their_existing_dependencies() {
    let actual = super::load(&repo::root()).unwrap();
    let layers = actual.layers.iter().filter(|r| r.krate == "ui").collect();
    let check = modules::CrateRules { layers, ..Default::default() };
    for (module, allowed) in [
        ("netio::port::latency", "perfguard::Clock"),
        ("netio::port::latency", "perfguard::ring::Ring"),
        ("netio::port::http::head", "store::MAX_WAVEFORM_BYTES"),
    ] {
        assert!(modules::check(&[source(module, &format!("use crate::{allowed};"))], &check).is_empty());
        for forbidden in ["ui::Ui", "scpi::Scpi", "model::Model", "system::Device", "store::set"] {
            assert!(!modules::check(&[source(module, &format!("use crate::{forbidden};"))], &check).is_empty());
        }
    }
    for module in ["netio", "netio::port::wake"] {
        assert!(!modules::check(&[source(module, "use crate::system::wake::wait;")], &check).is_empty());
    }
}

#[test]
fn network_ports_cannot_contain_unsafe() {
    let actual = super::load(&repo::root()).unwrap();
    for module in ["netio", "netio::port::wake"] {
        let measured = crate::size::measure("ui", &[source(module, "fn wait() { unsafe {} }")]);
        let violations = crate::steps::unsafe_now(&measured, &actual.unsafe_paths);
        assert_eq!(violations.0.values().copied().sum::<u64>(), 1, "{module}");
    }
}
