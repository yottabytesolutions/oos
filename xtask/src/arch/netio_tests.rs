// SPDX-License-Identifier: GPL-2.0-or-later
//! Exercise the actual M15a boundary rules, including forbidden dependency imports.
use super::{modules, rules};
use crate::{repo, sources::Source};

fn source(module: &str, text: &str) -> Source {
    Source {
        rel: format!("ui/src/{module}.rs"),
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
        &[source("netio", "use tokio::net::TcpStream; use crate::netport::RemoteRequest;")],
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
