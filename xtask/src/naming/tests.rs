// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn findings(text: &str) -> Ratchet {
    let vocabulary = Vocabulary::parse(include_str!("../../../docs/architecture/glossary.md")).unwrap();
    let mut declarations =
        Declarations { vocabulary: &vocabulary, file: "ui/src/example.rs", findings: Ratchet::default() };
    declarations.visit_file(&syn::parse_file(text).unwrap());
    declarations.findings
}

#[test]
fn words_preserve_acronyms_digits_and_raw_identifiers() {
    assert_eq!(split_words("FFTWindow"), ["FFT", "Window"]);
    assert_eq!(split_words("HttpRequest"), ["Http", "Request"]);
    assert_eq!(split_words("I2cChannel"), ["I2c", "Channel"]);
    assert_eq!(split_words("r#process_waveform"), ["process", "waveform"]);
}

#[test]
fn vague_words_are_components_not_substrings() {
    for name in [
        "Manager",
        "RecordHelper",
        "RecordManagers",
        "Util",
        "Utils",
        "Misc",
        "TraceData",
        "Info",
        "Handler",
        "Stuff",
        "Thing",
        "ImplRecord",
    ] {
        assert!(findings(&format!("struct {name};")).0.keys().any(|key| key.ends_with(" vague")), "{name}");
    }
    assert!(findings("struct Database; struct Information; fn configure_waveform() {}").0.is_empty());
}

#[test]
fn generic_verbs_need_a_domain_noun() {
    let bad = findings("fn do_work() {} fn handle_next() {} fn process_all() {}");
    assert_eq!(bad.0.keys().filter(|key| key.ends_with("domain-verb")).count(), 3);
    assert!(findings("fn do_sweep() {} fn handle_requests() {} fn process_waveform() {}").0.is_empty());
}

#[test]
fn generic_verbs_accept_glossary_nouns_with_changed_plural_endings() {
    assert!(findings("fn process_batches() {} fn handle_queries() {}").0.is_empty());
}

#[test]
fn glossary_synonyms_apply_to_types_and_aliases() {
    let bad = findings("struct Capture; enum Wave { A } type SweepBundle = (); trait Scan {}");
    assert_eq!(bad.0.len(), 4);
    assert!(findings("fn capture() {} struct Record; struct Sweep; struct Batch; struct Deposit; struct Picture; struct Waveform; struct Channel; struct Trace;").0.is_empty());
}

#[test]
fn acronyms_and_contractions_use_the_glossary_allowlist() {
    let bad = findings("struct XYZRecord; struct Wfm; fn recv_buf() {}");
    assert_eq!(bad.0.len(), 4);
    assert!(findings("struct DmaChannel; struct HttpRequest; fn get_id() {} fn write_csv() {} fn rms() {}")
        .0
        .is_empty());
}

#[test]
fn visits_inline_declarations_and_inherent_methods_but_excludes_tests_and_trait_impls() {
    let bad = findings("mod acquisition { struct RecordHelper; impl RecordHelper { fn handle_next() {} } } trait Port { fn do_work(); }");
    assert_eq!(bad.0.len(), 3);
    assert!(findings(
        r"
        #[cfg(all(test, unix))] mod tests { struct Helper; fn do_work() {} }
        #[test] fn process_all() {}
        struct Record;
        impl External for Record { fn do_work() {} }
        impl Record { #[cfg(test)] fn handle_next() {} }
    "
    )
    .0
    .is_empty());
}

#[test]
fn ratchet_rejects_new_names_and_growth_but_accepts_cleanup() {
    let old = findings("struct RecordHelper;");
    assert_eq!(old.compare(&findings("struct RecordHelper; struct SweepHelper;")).new.len(), 1);
    assert_eq!(old.compare(&findings("mod a { struct RecordHelper; } mod b { struct RecordHelper; }")).grown.len(), 1);
    assert_eq!(old.compare(&findings("struct Record;")).shrunk.len(), 1);
    assert!(Vocabulary::parse(" Empty glossary").is_err());
}

#[test]
fn mandated_trait_names_do_not_hide_local_declarations() {
    let bad = findings("impl External for Record { fn mandated_name() { struct Helper; fn do_work() {} } }");
    assert_eq!(bad.0.len(), 2);
    assert!(bad.0.keys().any(|key| key.ends_with("Helper vague")));
}

#[test]
fn visits_declarations_in_every_trait_implementation_member() {
    for member in [
        "const HELPER: () = { struct Helper; fn handle_next() {} };",
        "type Helper = [(); { struct Helper; fn handle_next() {} 0 }];",
        "fn do_work() -> [(); { struct Helper; fn handle_next() {} 0 }] { [] }",
    ] {
        assert_eq!(
            findings(&format!("impl External for Record {{ {member} }}")).0,
            findings("struct Helper; fn handle_next() {}").0,
            "{member}"
        );
    }
}

#[test]
fn trait_members_do_not_exempt_nested_inherent_members() {
    let bad = findings(
        r"
        impl External for Record {
            const VALUE: () = {
                struct Record;
                impl Record { fn handle_next() {} type Helper = (); }
            };
        }
        ",
    );
    assert_eq!(bad.0, findings("fn handle_next() {} type Helper = ();").0);
}

#[test]
fn excludes_test_only_trait_implementation_members() {
    assert!(findings(
        r"
        impl External for Record {
            #[cfg(test)] const VALUE: () = { struct Helper; fn handle_next() {} };
            #[cfg(test)] type Helper = [(); { struct Helper; fn handle_next() {} 0 }];
            #[cfg(test)] fn do_work() { struct Helper; fn handle_next() {} }
        }
        ",
    )
    .0
    .is_empty());
}

#[test]
fn checks_nested_declarations_only_in_production_containers() {
    let declarations = "struct Helper; fn do_work() {}";
    let expected = findings(declarations).0;
    for container in [
        "fn configure() { GATE { DECLARATIONS } }",
        "fn configure() { GATE let _ = { DECLARATIONS }; }",
        "fn configure() { match () { GATE () => { DECLARATIONS }, _ => {} } }",
        "fn configure() { let _ = Record { GATE channel: { DECLARATIONS } }; }",
        "struct Record { GATE channel: [(); { DECLARATIONS 0 }] }",
        "enum Waveform { GATE Channel = { DECLARATIONS 0 } }",
    ] {
        let source = container.replace("DECLARATIONS", declarations);
        for gate in ["#[cfg(test)]", "#[cfg(all(test, unix))]"] {
            assert!(findings(&source.replace("GATE", gate)).0.is_empty(), "{gate} in {container}");
        }
        for gate in ["", "#[cfg(any(test, unix))]"] {
            assert_eq!(findings(&source.replace("GATE", gate)).0, expected, "{gate} in {container}");
        }
    }
}
