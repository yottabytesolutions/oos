// SPDX-License-Identifier: GPL-2.0-or-later

use super::{busy_answer, Canvas, Response, Ui, SHOT_WAITING};
use crate::{
    json::J,
    scpi::{Program, Scpi},
    system::readback::Request,
};

#[derive(Default)]
pub(super) struct Requests(Vec<(Response, Pending)>);

pub(super) enum Pending {
    Bmp(Request),
    RawPng(Request),
    Png(Request, J),
    Scpi(Box<Program>, J),
}

impl Requests {
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    pub(super) fn push(&mut self, reply: Response, pending: Pending) {
        if self.0.len() >= SHOT_WAITING {
            busy_answer(&reply);
        } else {
            self.0.push((reply, pending));
        }
    }

    pub(super) fn poll(&mut self, ui: &mut Ui, cv: &Canvas, scpi: &mut Scpi) {
        self.0.retain_mut(|(reply, pending)| {
            if reply.closed() {
                return false;
            }
            let bmp = matches!(pending, Pending::Bmp(_));
            let response = match pending {
                Pending::Bmp(request) | Pending::RawPng(request) => {
                    let Some(readback) = scpi.readback() else { return false };
                    let Some(pixels) = request.pixels(readback) else { return true };
                    if bmp {
                        ("image/bmp", crate::scpi::bmp(pixels))
                    } else {
                        ("image/png", crate::store::png_of(pixels))
                    }
                }
                Pending::Png(request, id) => {
                    let Some(readback) = scpi.readback() else { return false };
                    let Some(pixels) = request.pixels(readback) else { return true };
                    ("application/json", rpc_result(id.clone(), screenshot(pixels)).into_bytes())
                }
                Pending::Scpi(program, id) => {
                    let Some(result) = scpi.poll_message(ui, cv, program) else { return true };
                    ("application/json", rpc_result(id.clone(), scpi_result(result)).into_bytes())
                }
            };
            if !reply.content("200 OK", response.0, response.1) {
                busy_answer(reply);
            }
            false
        });
    }
}

pub(super) enum McpReply {
    Now(Option<String>),
    Later(Pending),
}

fn rpc_result(id: J, result: J) -> String {
    J::obj(&[("jsonrpc", J::s("2.0")), ("id", id), ("result", result)]).to_string()
}

fn scpi_result(reply: Result<(Vec<u8>, bool), &'static str>) -> J {
    let content = |s: &str| J::Arr(vec![J::obj(&[("type", J::s("text")), ("text", J::s(s))])]);
    match reply {
        Ok((bytes, has_reply)) => {
            let text = if bytes.is_empty() && !has_reply {
                "ok"
            } else {
                std::str::from_utf8(&bytes).unwrap_or("(binary response)")
            };
            J::obj(&[("content", content(text))])
        }
        Err(error) => J::obj(&[("isError", J::Bool(true)), ("content", content(error))]),
    }
}

fn mcp_scpi(scpi: &mut Scpi, ui: &mut Ui, cv: &Canvas, text: &str, id: J) -> McpReply {
    let mut program = Program::new(text);
    match scpi.poll_message(ui, cv, &mut program) {
        Some(result) => McpReply::Now(Some(rpc_result(id, scpi_result(result)))),
        None => McpReply::Later(Pending::Scpi(Box::new(program), id)),
    }
}

fn screenshot(cv: &Canvas) -> J {
    J::obj(&[(
        "content",
        J::Arr(vec![J::obj(&[
            ("type", J::s("image")),
            ("mimeType", J::s("image/png")),
            ("data", J::Str(super::base64(&crate::store::png_of(cv)))),
        ])]),
    )])
}

/// Immediate MCP adapter for in-process callers.
pub fn mcp(body: &str, ui: &mut Ui, cv: &Canvas, scpi: &mut Scpi) -> Option<String> {
    match mcp_message(body, ui, cv, scpi, false) {
        McpReply::Now(response) => response,
        McpReply::Later(_) => unreachable!("the immediate adapter disables captures"),
    }
}

pub(super) fn mcp_reply(body: &str, ui: &mut Ui, cv: &Canvas, scpi: &mut Scpi) -> McpReply {
    mcp_message(body, ui, cv, scpi, true)
}

/// One MCP JSON-RPC message. Notifications get a 202 response and no body.
fn mcp_message(body: &str, ui: &mut Ui, cv: &Canvas, scpi: &mut Scpi, deferred: bool) -> McpReply {
    use crate::json::parse;
    let err = |id: J, code: f64, msg: &str| {
        J::obj(&[
            ("jsonrpc", J::s("2.0")),
            ("id", id),
            ("error", J::obj(&[("code", J::Num(code)), ("message", J::s(msg))])),
        ])
        .to_string()
    };
    let Some(req) = parse(body) else { return McpReply::Now(Some(err(J::Null, -32700.0, "parse error"))) };
    let Some(id) = req.get("id").cloned() else { return McpReply::Now(None) };
    let method = req.get("method").and_then(J::str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(J::Null);
    let result = match method {
        "initialize" => {
            let ver = params.get("protocolVersion").and_then(J::str).unwrap_or("2025-06-18").to_string();
            J::obj(&[
                ("protocolVersion", J::Str(ver)),
                ("capabilities", J::obj(&[("tools", J::obj(&[]))])),
                ("serverInfo", J::obj(&[("name", J::s("oos-ui")), ("version", J::s(env!("CARGO_PKG_VERSION")))])),
                ("instructions", J::s("Rigol DHO924S running OOS. Use the scpi tool with Rigol-style commands (e.g. :TIM:SCAL 1e-3, :CHAN1:SCAL 0.5, :MEAS:ITEM? VPP,CHAN1, :WAV:DATA?) and screenshot to see the screen.")),
            ])
        }
        "ping" => J::obj(&[]),
        "tools/list" => tool_list(),
        "tools/call" => match params.get("name").and_then(J::str) {
            Some("scpi") => {
                let c = params.get("arguments").and_then(|a| a.get("command")).and_then(J::str).unwrap_or("");
                if deferred {
                    return mcp_scpi(scpi, ui, cv, c, id);
                }
                scpi_result(scpi.exec_message_checked(ui, cv, c))
            }
            Some("screenshot") => {
                if let Some(readback) = scpi.readback().filter(|_| deferred) {
                    return McpReply::Later(Pending::Png(readback.request(), id));
                }
                screenshot(cv)
            }
            _ => return McpReply::Now(Some(err(id, -32602.0, "unknown tool"))),
        },
        _ => return McpReply::Now(Some(err(id, -32601.0, "method not found"))),
    };
    McpReply::Now(Some(rpc_result(id, result)))
}

fn tool_list() -> J {
    let schema =
        |props: J, req: Vec<J>| J::obj(&[("type", J::s("object")), ("properties", props), ("required", J::Arr(req))]);
    J::obj(&[(
        "tools",
        J::Arr(vec![
            J::obj(&[
                ("name", J::s("scpi")),
                ("description", J::s("Run one SCPI program message on the oscilloscope (commands separated by ';'); returns the response of the queries.")),
                ("inputSchema", schema(J::obj(&[("command", J::obj(&[("type", J::s("string"))]))]), vec![J::s("command")])),
            ]),
            J::obj(&[
                ("name", J::s("screenshot")),
                ("description", J::s("The current oscilloscope screen as a PNG image.")),
                ("inputSchema", schema(J::obj(&[]), vec![])),
            ]),
        ]),
    )])
}
