// SPDX-License-Identifier: GPL-2.0-or-later

use std::path::Path;

struct Case {
    name: String,
    kind: String,
    script: String,
    provenance: String,
}

pub fn inventory(out: &Path, smoke: bool) -> std::path::PathBuf {
    std::fs::create_dir_all(out).expect("golden test fixture must be present, UTF-8 and writable");
    let mut cases = demo_scripts();
    cases.extend(matrix());
    if smoke {
        cases.retain(|c| smoke_case(&c.name));
        assert_eq!(cases.len(), 20, "smoke routes must remain explicit");
    }
    assert!(!cases.is_empty());
    let mut index = String::new();
    for c in cases {
        let dir = out.join(&c.name);
        std::fs::create_dir_all(&dir).expect("golden test fixture must be present, UTF-8 and writable");
        let script = dir.join("script.txt");
        std::fs::write(&script, &c.script).expect("golden test fixture must be present, UTF-8 and writable");
        let shots = c.script.lines().filter_map(|l| l.strip_prefix("shot ")).collect::<Vec<_>>();
        assert!(!shots.is_empty(), "{}", c.name);
        index.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            c.name,
            c.kind,
            script.display(),
            shots.join(","),
            c.provenance
        ));
    }
    let path = out.join("inventory.tsv");
    std::fs::write(&path, index).expect("golden test fixture must be present, UTF-8 and writable");
    path
}

fn demo_scripts() -> Vec<Case> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut files = std::fs::read_dir(&root)
        .expect("golden test fixture must be present, UTF-8 and writable")
        .map(|p| p.expect("golden test fixture must be present, UTF-8 and writable").path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect::<Vec<_>>();
    files.sort();
    let mut result = Vec::new();
    for size in ["normal", "large", "larger"] {
        for file in &files {
            let text = std::fs::read_to_string(file).expect("golden test fixture must be present, UTF-8 and writable");
            let script = text.lines().map(rewrite).collect::<Vec<_>>().join("\n");
            result.push(Case {
                name: format!(
                    "demo/{}/{}",
                    size,
                    file.file_stem()
                        .expect("golden test fixture must be present, UTF-8 and writable")
                        .to_str()
                        .expect("golden test fixture must be present, UTF-8 and writable")
                ),
                kind: "demo".into(),
                script: format!("textsize {}\n{}\n", size, script),
                provenance: format!(
                    "script={};text={};acq.pipeline=batch;measurements=script",
                    file.file_name()
                        .expect("golden test fixture must be present, UTF-8 and writable")
                        .to_str()
                        .expect("golden test fixture must be present, UTF-8 and writable"),
                    size
                ),
            });
        }
    }
    result
}

fn rewrite(line: &str) -> String {
    if let Some(path) = line.strip_prefix("shot ") {
        return format!(
            "shot {}.png",
            Path::new(path)
                .file_stem()
                .expect("golden test fixture must be present, UTF-8 and writable")
                .to_str()
                .expect("golden test fixture must be present, UTF-8 and writable")
        );
    }
    line.replace("$S", ".")
}

fn recipe(name: &str) -> String {
    let text = include_str!("../m1-modes.recipes");
    let marker = format!("# [{}]\n", name);
    let body = text.split_once(&marker).unwrap_or_else(|| panic!("recipe {} missing", name)).1;
    body.split("# [").next().expect("golden test fixture must be present, UTF-8 and writable").trim().to_string() + "\n"
}

fn mode(kind: &str, name: &str, view: (&str, usize, &str), settings: &str, shots: &str) -> Case {
    let (dots, channels, time) = view;
    let mut script = String::new();
    for c in 2..=channels {
        script.push_str(&format!("key ch{}\n", c));
    }
    script.push_str(&format!(
        "menu Horizontal\nset Time/div = {}\nmenu Display\nset Dots = {}\n",
        time.replace(" us", " µs"),
        dots
    ));
    script.push_str(settings);
    script.push_str("menu close\n");
    script.push_str(&recipe(shots));
    Case {
        name: format!(
            "{}/{}/{}-{}ch-{}",
            if kind == "demo" { "demo/matrix" } else { "batch" },
            name,
            dots,
            channels,
            time.replace(' ', "")
        ),
        kind: kind.into(),
        script,
        provenance: format!(
            "fixture=m1-v1;source={};dots={};channels={};tdiv={};measurements={};settings={};recipe={}",
            kind,
            dots,
            channels,
            time,
            if kind == "demo" { "default" } else { "off" },
            settings.replace('\n', ";"),
            shots
        ),
    }
}

fn matrix() -> Vec<Case> {
    let mut cases = Vec::new();
    for dots in ["Off", "On", "Auto"] {
        for channels in [1, 2, 4] {
            for time in ["200 ns", "200 us"] {
                cases.push(mode("demo", "baseline", (dots, channels, time), "", "baseline"));
            }
            for kind in ["raw", "codes", "pairs", "deep-pairs"] {
                cases.push(mode(kind, kind, (dots, channels, "200 us"), "", "baseline"));
            }
        }
    }
    rates(&mut cases);
    persistence(&mut cases);
    persistence_routes(&mut cases);
    records(&mut cases);
    cases
}

fn rates(cases: &mut Vec<Case>) {
    for (kind, times) in [
        ("raw", vec!["200 ns", "5 us"]),
        ("codes", vec!["200 ns", "5 us"]),
        ("pairs", vec!["200 us"]),
        ("deep-pairs", vec!["1 ms"]),
    ] {
        for time in times {
            cases.push(mode(kind, &format!("rate-{}", kind), ("Off", 1, time), "", "baseline"));
        }
    }
}

fn persistence(cases: &mut Vec<Case>) {
    for kind in ["demo", "raw"] {
        for channels in [1, 4] {
            for dots in ["Off", "On"] {
                for persist in ["Min", "500 ms", "10 s", "Infinite"] {
                    let settings = format!("set Persistence = {}\n", persist);
                    cases.push(mode(
                        kind,
                        &format!("persist-{}", persist.replace(' ', "")),
                        (dots, channels, "200 us"),
                        &settings,
                        "later",
                    ));
                }
            }
            for grade in ["Off", "On"] {
                for persist in ["Min", "500 ms", "Infinite"] {
                    let settings = format!("set Persistence = {}\nset Colour grade = {}\n", persist, grade);
                    cases.push(mode(
                        kind,
                        &format!("grade-{}-{}", grade, persist.replace(' ', "")),
                        ("Off", channels, "200 us"),
                        &settings,
                        "later",
                    ));
                }
            }
        }
    }
}

fn records(cases: &mut Vec<Case>) {
    for dots in ["Off", "On", "Auto"] {
        for channels in [1, 4] {
            for time in ["5 us", "200 us"] {
                cases.push(mode(
                    "raw",
                    "average",
                    (dots, channels, time),
                    "menu Acquire\nset Mode = Average\nset Averages = 16\n",
                    "later",
                ));
                for persist in ["Min", "500 ms"] {
                    let settings = format!("set Persistence = {}\nmenu Horizontal\nset Zoom = On\n", persist);
                    cases.push(mode(
                        "raw",
                        &format!("zoom-{}", persist.replace(' ', "")),
                        (dots, channels, time),
                        &settings,
                        "later",
                    ));
                }
            }
        }
    }
    cases.push(mode(
        "hw-average",
        "pre-averaged",
        ("Off", 1, "200 us"),
        "menu Acquire\nset Mode = Average\nset Averages = 16\n",
        "later",
    ));
    for kind in ["roll", "scan"] {
        for dots in ["Off", "On"] {
            for channels in [1, 4] {
                cases.push(mode(kind, kind, (dots, channels, "200 ms"), "", "sweeps"));
            }
        }
    }
    cases.push(mode("raw", "frozen", ("Off", 1, "200 us"), "", "frozen"));
}

#[test]
fn inventory_is_unique_and_every_case_has_captures() {
    let inputs = super::isolated_inputs();
    let path = inventory(&inputs.path("inventory"), false);
    let text = std::fs::read_to_string(path).expect("golden test fixture must be present, UTF-8 and writable");
    assert!(text.lines().count() > 200);
    let names = text
        .lines()
        .map(|l| l.split('\t').next().expect("golden test fixture must be present, UTF-8 and writable"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), text.lines().count());
}

fn smoke_case(name: &str) -> bool {
    [
        "demo/normal/basic",
        "demo/large/menus",
        "demo/larger/axes",
        "demo/normal/decode",
        "demo/normal/xy",
        "batch/pre-averaged/Off-1ch-200us",
        "demo/normal/persist",
        "demo/normal/colorgrade",
        "batch/raw/Off-1ch-200us",
        "batch/raw/Auto-1ch-200us",
        "batch/rate-codes/Off-1ch-5us",
        "batch/rate-codes/Off-1ch-200ns",
        "batch/codes/On-4ch-200us",
        "batch/pairs/Off-1ch-200us",
        "batch/deep-pairs/Off-4ch-200us",
        "batch/average/Off-1ch-5us",
        "batch/zoom-500ms/Off-4ch-200us",
        "batch/frozen/Off-1ch-200us",
        "batch/roll/On-4ch-200ms",
        "batch/scan/Off-1ch-200ms",
    ]
    .contains(&name)
}

fn persistence_routes(cases: &mut Vec<Case>) {
    for kind in ["codes", "pairs", "deep-pairs"] {
        for dots in ["Off", "On"] {
            for channels in [1, 4] {
                for grade in ["Off", "On"] {
                    cases.push(mode(
                        kind,
                        &format!("{}-grade-{}-500ms", kind, grade),
                        (dots, channels, "200 us"),
                        &format!("set Persistence = 500 ms\nset Colour grade = {}\n", grade),
                        "later",
                    ));
                }
            }
        }
    }
    for kind in ["demo", "raw"] {
        for dots in ["Off", "On"] {
            for channels in [1, 4] {
                cases.push(mode(
                    kind,
                    "persist-500ms",
                    (dots, channels, "200 ns"),
                    "set Persistence = 500 ms\n",
                    "later",
                ));
            }
        }
    }
}

#[test]
fn smoke_inventory_covers_every_execution_route() {
    let inputs = super::isolated_inputs();
    let path = inventory(&inputs.path("smoke"), true);
    assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 20);
}
