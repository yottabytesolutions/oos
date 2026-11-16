// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn empty_mcp_queries_stay_empty_and_commands_answer_ok() {
    let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
    let cv = Canvas::new(1, 1);
    let mut scpi = crate::scpi::Scpi::open(0, false);
    scpi.exec_line(&mut ui, &cv, ":BODE:GEN \"\"");
    for (command, expected) in [
        (":ACQuire:SEGMents:TIMes?", ""),
        (":BODE:GEN?", ""),
        (":ACQ:SEGM:TIM?;*OPC?", ";1"),
        ("*OPC?", "1"),
        ("*CLS", "ok"),
    ] {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"scpi","arguments":{{"command":"{command}"}}}}}}"#
        );
        let reply = crate::json::parse(&mcp(&body, &mut ui, &cv, &mut scpi).unwrap()).unwrap();
        let Some(crate::json::J::Arr(content)) = reply.get("result").and_then(|r| r.get("content")) else {
            panic!("missing MCP content: {reply:?}");
        };
        assert_eq!(content[0].get("text").and_then(crate::json::J::str), Some(expected), "{command}");
    }
}
