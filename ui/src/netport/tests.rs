// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn owned_application_routes_are_distinct_from_scpi_programs() {
    let payload = Payload::new(b"Time (s),Volts\n0,0\n1,1\n".to_vec()).ok().unwrap();
    let fixtures = [
        ("GET /ui-state", RemoteRequest::UiState),
        ("POST /waveform", RemoteRequest::ImportWaveform { slot: 2, name: "wave.csv".into(), payload }),
        ("GET /wave.csv", RemoteRequest::ExportWaveform { format: WaveFormat::Csv }),
        ("GET /wave.bin", RemoteRequest::ExportWaveform { format: WaveFormat::Binary }),
        ("GET /la.csv", RemoteRequest::ExportLogicCsv),
        ("GET /stamp", RemoteRequest::Stamp),
        ("GET /update", RemoteRequest::UpdateStatus),
        (
            "POST /cmd",
            RemoteRequest::Cmd { lines: vec!["key RUN".into(), "workflow sim".into()], origin: Origin::Http },
        ),
        (
            "FIFO",
            RemoteRequest::Cmd { lines: vec!["workflow sim".into(), "shot image.png".into()], origin: Origin::Local },
        ),
    ];
    for (route, request) in fixtures {
        match (route, request) {
            ("GET /ui-state", RemoteRequest::UiState)
            | ("GET /la.csv", RemoteRequest::ExportLogicCsv)
            | ("GET /stamp", RemoteRequest::Stamp)
            | ("GET /update", RemoteRequest::UpdateStatus) => {}
            ("POST /waveform", RemoteRequest::ImportWaveform { slot, name, payload }) => {
                assert_eq!((slot, name.as_str()), (2, "wave.csv"));
                assert!(payload.bytes().starts_with(b"Time"));
            }
            ("GET /wave.csv", RemoteRequest::ExportWaveform { format: WaveFormat::Csv })
            | ("GET /wave.bin", RemoteRequest::ExportWaveform { format: WaveFormat::Binary }) => {}
            ("POST /cmd" | "FIFO", RemoteRequest::Cmd { lines, origin }) => {
                if origin == Origin::Http {
                    assert_eq!(lines, ["key RUN", "workflow sim"]);
                } else {
                    assert_eq!(lines, ["workflow sim", "shot image.png"]);
                }
                assert_eq!(origin, if route == "FIFO" { Origin::Local } else { Origin::Http });
                assert!(crate::cmd::remote_ok(&lines[0]));
            }
            _ => panic!("wrong application route {route}"),
        }
    }
}

#[test]
fn local_command_order_and_shot_outcomes_keep_http_privileges_separate() {
    let mut ui = crate::Ui::for_tests(crate::Source::Demo { t: 0.0 });
    assert!(!crate::cmd::remote_ok("shot image.png"));
    assert!(crate::cmd::remote_ok("workflow sim"));
    assert!(matches!(crate::cmd::run(&mut ui, "workflow sim"), crate::cmd::Out::None));
    assert!(matches!(crate::cmd::run(&mut ui, "shot image.png"), crate::cmd::Out::Shot(path) if path == "image.png"));
}

#[test]
fn signed_upload_permit_rejects_overload_and_releases_on_cancellation() {
    let admission = UploadAdmission::default();
    assert!(admission.admit(UPLOAD_BYTES + 1).is_none());
    let pending = admission.admit(UPLOAD_BYTES).unwrap();
    assert!(admission.admit(1).is_none());
    assert!(admission.admit(1).is_none(), "rejection must not release the current owner's permit");
    let job = UploadJob::new(42);
    assert_eq!(job.id(), 42, "only the owned job identity enters application intake");
    drop(pending);
    assert!(admission.admit(1).is_some());
}
