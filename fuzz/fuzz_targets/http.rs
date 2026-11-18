// SPDX-License-Identifier: GPL-2.0-or-later
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some((&flags, data)) = data.split_first() else { return };
    let text = String::from_utf8_lossy(data);
    let mut lines = text.lines();
    let mut builder = hyper::Request::builder()
        .uri(["/update", "/waveform", "/mcp", "/cmd"][usize::from(flags & 3)])
        .method(if flags & 4 == 0 { "GET" } else { "POST" })
        .header("host", lines.next().unwrap_or(""));
    if flags & 8 != 0 {
        builder = builder.header("content-length", lines.next().unwrap_or(""));
    }
    if flags & 16 != 0 {
        builder = builder.header("transfer-encoding", lines.next().unwrap_or(""));
    }
    if flags & 32 != 0 {
        builder = builder.header("origin", lines.next().unwrap_or(""));
    }
    if flags & 64 != 0 {
        builder = builder.header("sec-fetch-site", lines.next().unwrap_or(""));
    }
    let Ok(request) = builder.body(()) else { return };
    let head = oos_ui::http::parse_head(&request);
    for peer_ok in [false, true] {
        let _ = oos_ui::http::refuse_early(&head, request.headers(), peer_ok);
    }
});
