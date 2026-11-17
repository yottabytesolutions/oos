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
