// SPDX-License-Identifier: GPL-2.0-or-later
use std::time::Duration;
/// Longest request: `POST /update` (a package is ~31 MB, the image at most 64 MiB), every other request.
pub(crate) const MAX_UPDATE: usize = 70 << 20;
pub(crate) const MAX_WAVEFORM: usize = crate::store::MAX_WAVEFORM_BYTES;
pub(crate) const MAX_OTHER: usize = 1 << 20;
/// Requests being read at once (more are answered 503 at accept).
pub(crate) const MAX_PENDING: usize = 8;
/// Longest request head; a longer one is answered 431.
pub(crate) const MAX_HEAD: usize = 16384;

/// Request limits enforced by IO, even while frame intake is stalled.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// no data for this long: 408
    pub(crate) idle: Duration,
    /// the whole request, head and body (the upload of a package over 10 Mbit/s takes ~30 s): 408
    pub(crate) deadline: Duration,
    /// a complete request the frame loop has not taken after this long: 503
    pub(crate) take: Duration,
}

pub(crate) const LIMITS: Limits =
    Limits { idle: Duration::from_secs(10), deadline: Duration::from_secs(120), take: Duration::from_secs(10) };

/// A body above this (an update package, a waveform) is read one at a time: a second one is answered 503 when its
/// head is in, so the readers hold at most one large body and `MAX_PENDING` small ones.
pub(crate) const LARGE_BODY: usize = MAX_OTHER;

/// The parts of a request head the reader needs (names without case).
#[derive(Debug, PartialEq)]
pub struct Head {
    pub(crate) post: bool,
    pub(crate) route: String,
    /// Content-Length; None without one
    pub(crate) len: Option<usize>,
    pub(crate) chunked: bool,
    pub(crate) expect_continue: bool,
}

pub fn parse_head(text: &str) -> Head {
    let mut words = text.split_whitespace();
    let post = words.next() == Some("POST");
    let path = words.next().unwrap_or("/");
    let route = path.split_once('?').map_or(path, |x| x.0).to_string();
    let len = header(text, "content-length").and_then(|v| v.parse::<usize>().ok());
    let chunked = header(text, "transfer-encoding").is_some_and(|v| !v.eq_ignore_ascii_case("identity"));
    let expect_continue = header(text, "expect").is_some_and(|v| v.eq_ignore_ascii_case("100-continue"));
    Head { post, route, len, chunked, expect_continue }
}

/// Why a request is answered before its body is read: the status line and text, or None to read it.
pub fn refuse_early(h: &Head, text: &str, peer_ok: bool) -> Option<(&'static str, &'static str)> {
    let limit = match (h.post, h.route.as_str()) {
        (true, "/update") => MAX_UPDATE,
        (true, "/waveform") => MAX_WAVEFORM,
        _ => MAX_OTHER,
    };
    if h.post && (h.chunked || h.len.is_none()) {
        return Some(("411 Length Required", "a body needs a Content-Length"));
    }
    if h.len.is_some_and(|n| n > limit) {
        return Some(("413 Payload Too Large", "request too large"));
    }
    if foreign(text, &h.route, h.post)
        || (matches!(h.route.as_str(), "/update" | "/mcp") && (h.post || h.route == "/mcp") && !peer_ok)
    {
        return Some(("403 Forbidden", "refused"));
    }
    None
}

/// The value of a header of the request head (name without case).
pub(crate) fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

/// A request a web page on another site made through the browser of the bench PC, not the page of the scope or a tool:
/// it may press keys or install a package without the owner's doing. The Host names the scope by address or local
/// name (a foreign name that resolves here is DNS rebinding), an Origin matches the Host, and the browser's own
/// Sec-Fetch-Site says the rest, also for the GET of an image tag, which has no Origin. A link to the page itself may
/// come from anywhere. curl and the like send neither header.
pub(crate) fn foreign(head: &str, route: &str, post: bool) -> bool {
    let header = |name: &str| {
        head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case(name).then(|| v.trim().to_ascii_lowercase())
        })
    };
    let name_of = |h: &str| {
        h.rsplit_once(':')
            .filter(|(n, p)| !n.ends_with(':') && p.bytes().all(|b| b.is_ascii_digit()))
            .map_or(h.to_string(), |(n, _)| n.to_string())
    };
    let host = header("host");
    if let Some(h) = &host {
        let n = name_of(h);
        let n = n.trim_matches(['[', ']']);
        let local =
            n.parse::<core::net::IpAddr>().is_ok() || n == "localhost" || !n.contains('.') || n.ends_with(".local");
        if !local {
            return true;
        }
    }
    if let Some(o) = header("origin") {
        let o = o.split_once("://").map_or(o.as_str(), |x| x.1).to_string();
        if host.as_deref() != Some(o.as_str()) {
            return true;
        }
    }
    let page = !post && matches!(route, "/" | "/style.css" | "/update" | "/lxi/identification");
    !page && matches!(header("sec-fetch-site").as_deref(), Some("cross-site" | "same-site"))
}
