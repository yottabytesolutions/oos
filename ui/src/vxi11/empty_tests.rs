// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn rpc(
    proc_: u32,
    args: &[u8],
    links: &mut Vec<Option<Link>>,
    scpi: &mut crate::scpi::Scpi,
    ui: &mut crate::ui::Ui,
) -> Vec<u8> {
    let cv = crate::gfx::Canvas::new(1, 1);
    core_async(&Call { xid: 1, prog: CORE_PROG, vers: 1, proc_, args }, links, &mut |p| scpi.poll_program(ui, &cv, p))
}

fn write(command: &str, links: &mut Vec<Option<Link>>, scpi: &mut crate::scpi::Scpi, ui: &mut crate::ui::Ui) {
    let mut args = Vec::new();
    for v in [0, 1000, 0, 8] {
        put(&mut args, v);
    }
    put_opaque(&mut args, command.as_bytes());
    let r = rpc(11, &args, links, scpi, ui);
    let mut x = Xdr::new(&r[24..]);
    assert_eq!((x.u32(), x.u32()), (Some(0), Some(command.len() as u32)));
}

#[test]
fn empty_queries_have_terminated_vxi_responses_and_commands_stay_silent() {
    let mut ui = crate::ui::Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
    let mut scpi = crate::scpi::Scpi::open(0, false);
    let mut links = vec![Some(Link::default())];
    write(":BODE:GEN \"\"", &mut links, &mut scpi, &mut ui);
    for (command, expected) in [
        (":ACQuire:SEGMents:TIMes?", &b"\n"[..]),
        (":BODE:GEN?", &b"\n"[..]),
        (":ACQ:SEGM:TIM?;*OPC?", &b";1\n"[..]),
        ("*CLS\n:BODE:GEN?\r\n*OPC?", &b"\n1\n"[..]),
        ("*CLS", &b""[..]),
    ] {
        write(command, &mut links, &mut scpi, &mut ui);
        let mut args = Vec::new();
        for v in [0, 1024, 1000, 0, 0, 0] {
            put(&mut args, v);
        }
        let r = rpc(12, &args, &mut links, &mut scpi, &mut ui);
        let mut x = Xdr::new(&r[24..]);
        assert_eq!(x.u32(), Some(if expected.is_empty() { 15 } else { 0 }), "{command}");
        assert_eq!(x.u32(), Some(if expected.is_empty() { 0 } else { 4 }));
        assert_eq!(x.opaque(), Some(expected));
        assert!(links[0].as_ref().unwrap().out.is_empty());
    }
}
