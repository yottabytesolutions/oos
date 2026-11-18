// SPDX-License-Identifier: GPL-2.0-or-later

use super::{pkg, update, Path, BIN, UPLOAD};

#[derive(Debug)]
pub(super) enum Reply {
    Accepted(String),
    Confirmation { package: String, running: String },
}

/// A mismatch requires a separate request; no uploaded bytes are retained while the user decides.
pub(super) fn confirmation(m: &pkg::Manifest, running: &str, any: bool) -> Result<Option<Reply>, String> {
    let package = update::models::package(m)?;
    let mismatch = update::models::check(m, running, true)?.is_some();
    Ok((mismatch && !any).then(|| Reply::Confirmation { package: package.into(), running: running.into() }))
}

pub(super) fn reply(result: Result<Reply, String>) -> (bool, &'static str, Vec<u8>) {
    use crate::json::J;
    let (refused, kind, text) = match result {
        Ok(Reply::Confirmation { package, running }) => (
            true,
            "application/json",
            J::obj(&[
                ("refused", J::Bool(true)),
                ("package_models", J::s(&package)),
                ("running_model", J::s(&running)),
            ])
            .to_string(),
        ),
        Ok(Reply::Accepted(text)) => (false, "text/plain", text),
        Err(e) => (true, "text/plain", format!("REFUSED: {}", e)),
    };
    (refused, kind, text.into_bytes())
}

/// The checks of an uploaded package that the frame loop can afford: header signature against `keys`, target and the
/// image length the header names (a truncated upload fails here). The image hash is `upload_store`'s.
#[cfg(test)]
pub(super) fn upload_header(body: &[u8], keys: &[[u8; 32]]) -> Result<pkg::Header, String> {
    upload_header_len(body, body.len(), keys)
}

pub(super) fn upload_header_len(body: &[u8], len: usize, keys: &[[u8; 32]]) -> Result<pkg::Header, String> {
    let h = pkg::verify_header(body, keys)?;
    let m = &h.manifest;
    if m.target != pkg::TARGET {
        return Err(format!("package is for '{}', this unit takes '{}'", m.target, pkg::TARGET));
    }
    let img = len.saturating_sub(h.data_offset) as u64;
    if img != m.size {
        return Err(format!("image is {} bytes, the header says {} (truncated upload?)", img, m.size));
    }
    Ok(h)
}

/// The whole check (`pkg::verify`: also the image hash), then the package to `path` for `oos-update install`. Off the
/// frame loop. Err: nothing written.
pub(super) fn upload_store(body: &[u8], keys: &[[u8; 32]], path: &Path) -> Result<(), String> {
    pkg::verify(body, keys)?;
    std::fs::write(path, body).map_err(|e| format!("{}: {}", path.display(), e))
}

/// Only an explicit, unique query option permits a foreign model package.
pub(super) fn any_model(query: &str) -> Result<bool, String> {
    let mut values = query.split('&').filter_map(|p| p.strip_prefix("any-model="));
    match (values.next(), values.next()) {
        (None, None) => Ok(false),
        (Some("1"), None) => Ok(true),
        _ => Err("use a single any-model=1 query option to override the unit check".into()),
    }
}

pub(super) fn install_args(any_model: bool) -> Vec<&'static str> {
    let mut args = vec!["install", UPLOAD, "--execute"];
    if any_model {
        args.push("--any-model");
    }
    args
}

/// Header acceptance precedes image verification and installation on both paths.
pub(super) fn accepted(m: &pkg::Manifest) -> String {
    let (start, next) = if update::kexec::announced(m) {
        (update::text::KEXEC_START, "restart")
    } else {
        (update::text::DIRECT_START, "power cycle")
    };
    format!("accepted, build {}: header signature ok; checking the image before installation. {} {} System > Update and this page show the state; check again after the {}.", update::short(&m.build), start, update::text::trial(), next)
}

/// Compact panel guidance; an installed but not yet running trial still needs its first ordinary start.
pub(super) fn trial_lines(st: &update::State, running: Option<&str>, cmdline: Option<&str>) -> Vec<String> {
    let started = st.trial.is_some_and(|n| n > 0)
        && st.current.as_ref().is_some_and(|c| running.is_some_and(|r| r.trim() == c.build.trim()));
    let start = if cmdline.is_some_and(|c| update::kexec::flags(c).is_some()) {
        "Restart refused or pending; power-cycle"
    } else if started {
        "Trial running; confirms automatically"
    } else {
        "Trial pending; power-cycle to start"
    };
    vec![start.into(), "After 20 s stable UI and startup done".into(), "Stable UI with startup stuck: 180 s".into()]
}

/// The web page /update: state and an upload form.
pub(super) fn page(last: &str) -> String {
    let status = std::process::Command::new(BIN)
        .args(["status", "--quick"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
        .unwrap_or_else(|e| format!("{}: {}", BIN, e));
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;");
    format!(
        r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>OOS update</title>
<style>{}</style></head>
<body><header><b>OOS update</b><nav><a href="/">Remote</a></nav></header>
<pre>{}</pre>
<p>Package (<code>.oosupd</code>, signed): <input type="file" id="f" accept=".oosupd"> <button onclick="up()">Install</button></p>
<pre id="r">{}</pre>
<p>{}<br>Without kexec: {}<br>{}<br>A failed trial restores the previous image when available; it does not restart. Follow the restore notice before power-cycling.</p>
<p>
Command line: <code>curl --max-time 300 --data-binary @oos-dev.oosupd http://&lt;scope&gt;:8080/update</code>
(the answer comes once the package has arrived and its header is checked; the image check and the install follow, their result shows here).</p>
<script>
async function up(){{const f=document.getElementById('f').files[0];if(!f)return;
const r=document.getElementById('r');const button=document.querySelector('button');button.disabled=true;
try{{r.textContent='uploading '+f.size+' bytes ...';
let response=await fetch('/update',{{method:'POST',body:f}});
if(response.status===409&&(response.headers.get('Content-Type')||'').includes('application/json')){{
const models=await response.json();
if(models.refused!==true)throw new Error('Invalid refusal response');
const warning="Package models '"+models.package_models+"', running device model '"+models.running_model+"'.";
r.textContent=warning;
if(!confirm(warning+' Install anyway? This override will be logged.')){{r.textContent+=' Cancelled; nothing installed.';return}}
response=await fetch('/update?any-model=1',{{method:'POST',body:f}});
}}
r.textContent=await response.text();if(!response.ok)return;setTimeout(()=>location.reload(),15000);
}}catch(e){{r.textContent='Upload failed: '+e.message}}finally{{button.disabled=false}}
}}
</script></body></html>"#,
        crate::http::STYLE,
        esc(&status),
        esc(last),
        update::text::KEXEC_START,
        update::text::DIRECT_START,
        update::text::trial()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_explicit_query_flag_reaches_the_install_child() {
        assert!(!any_model("").unwrap());
        assert!(!any_model("x=any-model=1").unwrap());
        assert!(!any_model("any-model").unwrap());
        assert!(any_model("any-model=1").unwrap());
        assert!(any_model("x=1&any-model=1").unwrap());
        for query in ["any-model=0", "any-model=yes", "any-model=1&any-model=0", "any-model=1&any-model=1"] {
            assert!(any_model(query).is_err());
        }
        assert_eq!(install_args(false), ["install", UPLOAD, "--execute"]);
        assert_eq!(install_args(true), ["install", UPLOAD, "--execute", "--any-model"]);
        let page = page("");
        assert!(page.contains("confirm(warning+"));
        assert!(page.contains("models.package_models") && page.contains("models.running_model"));
        assert!(!page.contains("type=\"checkbox\""));
        assert!(page.contains("?any-model=1"));
    }

    #[test]
    fn confirmation_names_both_units_and_requires_a_new_explicit_request() {
        let body = pkg::build_with(&[9; 32], "b", "k", b"image", &[("models", "other-unit")]).unwrap();
        let h = upload_header(&body, &[update::ed25519::public_key(&[9; 32])]).unwrap();
        let decision = confirmation(&h.manifest, "this-unit", false).unwrap().unwrap();
        let (refused, kind, text) = reply(Ok(decision));
        assert!(refused);
        assert_eq!(kind, "application/json");
        assert_eq!(
            String::from_utf8(text).unwrap(),
            "{\"refused\":true,\"package_models\":\"other-unit\",\"running_model\":\"this-unit\"}"
        );
        assert!(confirmation(&h.manifest, "other-unit", false).unwrap().is_none());
        assert!(confirmation(&h.manifest, "this-unit", true).unwrap().is_none());
        assert!(confirmation(&h.manifest, "invalid unit", true).is_err());
        let warning = update::models::check(&h.manifest, "this-unit", true).unwrap().unwrap();
        let (refused, _, text) = reply(Ok(Reply::Accepted(warning)));
        assert!(!refused);
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("--any-model") && text.contains("other-unit") && text.contains("this-unit"));
    }
}
