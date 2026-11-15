// SPDX-License-Identifier: GPL-2.0-or-later

use super::{pkg, update, Path, BIN, UPLOAD};

#[derive(Debug)]
pub(super) enum Reply {
    Accepted(String),
    Confirmation { package: String, running: String },
}

/// A mismatch requires a separate request; no uploaded bytes are retained while the user decides.
pub(super) fn confirmation(m: &pkg::Manifest, running: &str, any: bool) -> Result<Option<Reply>, String> {
    let package = update::unit::package(m)?;
    let running = update::unit::validate(running)?;
    Ok((package != running && !any).then(|| Reply::Confirmation { package: package.into(), running: running.into() }))
}

pub(super) fn reply(result: Result<Reply, String>) -> (bool, &'static str, Vec<u8>) {
    use crate::json::J;
    let (refused, kind, text) = match result {
        Ok(Reply::Confirmation { package, running }) => (
            true,
            "application/json",
            J::obj(&[("refused", J::Bool(true)), ("package_unit", J::s(&package)), ("running_unit", J::s(&running))])
                .to_string(),
        ),
        Ok(Reply::Accepted(text)) => (false, "text/plain", text),
        Err(e) => (true, "text/plain", format!("REFUSED: {}", e)),
    };
    (refused, kind, text.into_bytes())
}

/// The checks of an uploaded package that the frame loop can afford: header signature against `keys`, target and the
/// image length the header names (a truncated upload fails here). The image hash is `upload_store`'s.
pub(super) fn upload_header(body: &[u8], keys: &[[u8; 32]]) -> Result<pkg::Header, String> {
    let h = pkg::verify_header(body, keys)?;
    let m = &h.manifest;
    if m.target != pkg::TARGET {
        return Err(format!("package is for '{}', this unit takes '{}'", m.target, pkg::TARGET));
    }
    let img = body.len().saturating_sub(h.data_offset) as u64;
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

/// Only an explicit, unique query option permits a foreign unit package.
pub(super) fn any_unit(query: &str) -> Result<bool, String> {
    let mut values = query.split('&').filter_map(|p| p.strip_prefix("any-unit="));
    match (values.next(), values.next()) {
        (None, None) => Ok(false),
        (Some("1"), None) => Ok(true),
        _ => Err("use a single any-unit=1 query option to override the unit check".into()),
    }
}

pub(super) fn install_args(any_unit: bool) -> Vec<&'static str> {
    let mut args = vec!["install", UPLOAD, "--execute"];
    if any_unit {
        args.push("--any-unit");
    }
    args
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
<p>The new build starts on trial at the next power-on; a failed start writes the previous build back.
Command line: <code>curl --max-time 300 --data-binary @oos-dev.oosupd http://&lt;scope&gt;:8080/update</code>
(the answer comes once the package has arrived and its header is checked; the image check and the install follow, their result shows here).</p>
<script>
async function up(){{const f=document.getElementById('f').files[0];if(!f)return;
const r=document.getElementById('r');const button=document.querySelector('button');button.disabled=true;
try{{r.textContent='uploading '+f.size+' bytes ...';
let response=await fetch('/update',{{method:'POST',body:f}});
if(response.status===409&&(response.headers.get('Content-Type')||'').includes('application/json')){{
const units=await response.json();
if(units.refused!==true)throw new Error('Invalid refusal response');
const warning="Package unit '"+units.package_unit+"', running image unit '"+units.running_unit+"'.";
r.textContent=warning;
if(!confirm(warning+' Install anyway? This override will be logged.')){{r.textContent+=' Cancelled; nothing installed.';return}}
response=await fetch('/update?any-unit=1',{{method:'POST',body:f}});
}}
r.textContent=await response.text();if(!response.ok)return;setTimeout(()=>location.reload(),15000);
}}catch(e){{r.textContent='Upload failed: '+e.message}}finally{{button.disabled=false}}
}}
</script></body></html>"#,
        crate::http::STYLE,
        esc(&status),
        esc(last)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_explicit_query_flag_reaches_the_install_child() {
        assert!(!any_unit("").unwrap());
        assert!(!any_unit("x=any-unit=1").unwrap());
        assert!(!any_unit("any-unit").unwrap());
        assert!(any_unit("any-unit=1").unwrap());
        assert!(any_unit("x=1&any-unit=1").unwrap());
        for query in ["any-unit=0", "any-unit=yes", "any-unit=1&any-unit=0", "any-unit=1&any-unit=1"] {
            assert!(any_unit(query).is_err());
        }
        assert_eq!(install_args(false), ["install", UPLOAD, "--execute"]);
        assert_eq!(install_args(true), ["install", UPLOAD, "--execute", "--any-unit"]);
        let page = page("");
        assert!(page.contains("confirm(warning+"));
        assert!(page.contains("units.package_unit") && page.contains("units.running_unit"));
        assert!(!page.contains("type=\"checkbox\""));
        assert!(page.contains("?any-unit=1"));
    }

    #[test]
    fn confirmation_names_both_units_and_requires_a_new_explicit_request() {
        let body = pkg::build_with(&[9; 32], "b", "k", b"image", &[("unit", "other-unit")]).unwrap();
        let h = upload_header(&body, &[update::ed25519::public_key(&[9; 32])]).unwrap();
        let decision = confirmation(&h.manifest, "this-unit", false).unwrap().unwrap();
        let (refused, kind, text) = reply(Ok(decision));
        assert!(refused);
        assert_eq!(kind, "application/json");
        assert_eq!(
            String::from_utf8(text).unwrap(),
            "{\"refused\":true,\"package_unit\":\"other-unit\",\"running_unit\":\"this-unit\"}"
        );
        assert!(confirmation(&h.manifest, "other-unit", false).unwrap().is_none());
        assert!(confirmation(&h.manifest, "this-unit", true).unwrap().is_none());
        assert!(confirmation(&h.manifest, "invalid unit", true).is_err());
        let warning = update::unit::check(&h.manifest, "this-unit", true).unwrap().unwrap();
        let (refused, _, text) = reply(Ok(Reply::Accepted(warning)));
        assert!(!refused);
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("--any-unit") && text.contains("other-unit") && text.contains("this-unit"));
    }
}
