// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

#[cfg(target_os = "linux")]
pub(super) fn run(_a: &[String]) -> Result<(), String> {
    use oos_hal::dho::bus::{Bar1, Bus};
    let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
    for l in status_lines(&mut |o| bar.rd(o)) {
        println!("{}", l);
    }
    Ok(())
}

/// Interpretation of the read-only status registers.
pub(super) fn status_lines(rd: &mut dyn FnMut(u32) -> u32) -> Vec<String> {
    use oos_hal::dho::boot::DDR_INIT_PASS;
    let (cmin, cmax) = oos_hal::dho::boot::adc_clk_window();
    let v = rd(0x4000);
    let st = rd(0x4034);
    let ddr = rd(0x1008);
    let clk = rd(0x1210);
    let (last, first, play) = (rd(0x4010), rd(0x4014), rd(0x4080));
    let x = rd(oos_hal::dho::board::XADC_REG);
    let (t, vcc) = oos_hal::dho::board::xadc(x);
    vec![
        format!(
            "FPGA version 0x4000 = {:#010x} {}",
            v,
            if v == oos_hal::dho::bus::FPGA_VERSION { "(expected)" } else { "(UNEXPECTED)" }
        ),
        format!(
            "status 0x4034 = {:#010x}: FSM {}, {}",
            st,
            if st & 0x2000 != 0 { "stopped" } else { "running" },
            if st & 0x10 != 0 { "waiting for trigger" } else { "not waiting" }
        ),
        format!("DDR 0x1008 = {:#010x}: init {}", ddr, if ddr & DDR_INIT_PASS != 0 { "pass" } else { "NOT passed" }),
        format!(
            "ADC clock count 0x1210 = {} -> PLL {}",
            clk,
            if (cmin..=cmax).contains(&clk) { "locked" } else { "NOT locked / not running" }
        ),
        format!("XADC 0x1438 = {:#010x}: FPGA die {:.1} C, VCCINT {:.3} V", x, t, vcc),
        format!(
            "play window 0x4014..0x4010 = {}..{} ({} frames), 0x4080 = {:#010x} (done bit30 {})",
            first,
            last,
            last.wrapping_sub(first).wrapping_add(1),
            play,
            (play >> 30) & 1
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_interpretation() {
        let mut regs = std::collections::HashMap::from([
            (0x4000u32, 0xe207_2115u32),
            (0x4034, 0x2000),
            (0x1008, 1 << 29),
            (0x1210, 62500),
        ]);
        let l = status_lines(&mut |o| *regs.entry(o).or_insert(0));
        assert!(
            l[0].contains("expected") && l[1].contains("stopped") && l[2].contains("pass") && l[3].contains("locked")
        );
    }
}
