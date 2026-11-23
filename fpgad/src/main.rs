// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! oos-fpgad: OOS acquisition control and bring-up tool over `oos_hal` (replaces the C tool oos-hw step by step).
//!
//!   oos-fpgad model [--model NAME] [--limits FILE]   model, serial, capabilities and limits of the unit
//!                                          (/rigol/data/vendor.bin, /oos/limits.conf; docs/models.md), read only
//!   oos-fpgad plan [options]               print the complete acquisition sequence, no hardware access (runs anywhere)
//!   oos-fpgad run  [options] --execute     run it on the unit and write the frame to --out (Linux, FPGA up)
//!   oos-fpgad status                       read only: FPGA version, FSM, DDR init, ADC clock (PLL lock), play window
//!   oos-fpgad rd <offset> [count]          read BAR1 registers
//!   oos-fpgad wr <offset> <value> --execute   write one BAR1 register (0x1008 refused: BRAM fallback bit)
//!   oos-fpgad fpga-boot [--slot 0x400000] [--progress FILE]   only the BOOT command (start of rcS: the bitstream loads
//!                                          while Linux starts the rest); its uptime goes to /tmp/oos-fpga-booted
//!   oos-fpgad cold-boot [--execute] [--power-on [--slot 0x400000] [--booted]] [--ignore-clock] [--progress FILE]
//!                                          (refused while XDMA or the PCIe host is loaded: power cycle); --booted:
//!                                          BOOT was sent by `fpga-boot`, only the rest of the wait; --progress: start-up
//!                                          screen lines (oos_hal::bootstatus), then the temperature check
//!                                          without --execute: dry run against the unit model (runs anywhere);
//!                                          --execute: cold boot with the FPGA already on PCIe; --power-on: first the
//!                                          BOOT command (slots 0x0 / 0x400000 only) and the PCIe/xdma bring-up;
//!                                          --ignore-clock: 0x1210 out of range only warns, counter logged per step
//!   oos-fpgad pll [--reset | --r0 <val>]                LMX2582 table over GPIO (LE idles high first like spi2pll_lxm2582_gpio.ko;
//!                                          --reset: R0 RESET=1/0 first, as the TI datasheet asks), then reads 0x1210
//!   oos-fpgad pll --freq <Hz> --dry        table for another ADC clock (oos_hal::dho::lmx2582, pll.md) and its decode,
//!                                          no hardware access; without --dry only 1.25 GHz (others are untested)
//!   oos-fpgad gpio <line> <0|1> [--hold 60] [--force]  drive a GPIO line (by DT name) and keep it for --hold seconds;
//!                                          generator lines (afg_in1..5, FG_M2C_GPIO) only with a generator or --force
//!   oos-fpgad roll [run options] [--secs 3] [--scan] --execute
//!                                          roll mode (roll-mode.md): normal set-up, roll display chain, stream for
//!                                          --secs seconds, chunks (216 bytes: header + 100 words) to --out;
//!                                          --scan: scan bit instead of roll, logs 0x1204 / 0x40d0, restarts after done
//!   oos-fpgad selfcal entry|zero|vertical|all [--adc] --ch N [--var 0..3] [--tuple ig,vga,att,sga,sgar,mux,muxr] [--limit n]
//!                                          [--execute] [--save]
//!                                          self-calibration (docs/protocols/self-cal.md), inputs open: `entry` measures one
//!                                          gain configuration and compares it with the table, `zero` the AFE zero trims,
//!                                          `vertical` one bandwidth block; --save writes only to /oos/cal (never /rigol)
//!   oos-fpgad afg sine|square|ramp|pulse|noise|dc --freq 1000 --ampl 1 [--offset 0] [--duty 50] [--symmetry 50]
//!                 [--edge s] [--mod am|fm|pm --mod-rate Hz --depth % --dev Hz|deg --mod-shape sine|square|triangle|up|down]
//!                 [--execute] | afg off [--execute]
//!                                          built-in generator (afg.md): wave at Hz / Vpp / V with the factory
//!                                          cal_afg.hex (dc: level = --offset); dry run prints the writes
//!   oos-fpgad la status | la thr <pod1 V> <pod2 V> [--execute]
//!                                          logic analyser (la.md): probe, line states 0x12dc, thresholds 0x12d4
//!   oos-fpgad adc slots [--runs 4]         ADC measure unit (0x1040..0x1048): mean, min, max per slot and each slot's
//!                                          mean against the slot average in ADC steps (docs/fork/core-offset.md)
//!   oos-fpgad adc offset <core 0..7> <steps> [--execute]
//!                                          one RT8847 core offset register (0x14 + 8n) over 0x1034; dry run prints
//!                                          the word. Both refused while oos-ui runs (it owns the ADC and the measure unit)
//!   oos-fpgad temp                         read only: SoC thermal zones, FPGA die temperature and VCCINT (XADC 0x1438)
//!   oos-fpgad beep [once|long]             beeper (pwm-gpio, 2 kHz): 20 ms click or 4 s alarm
//!   oos-fpgad guard [--period 1] [--power-off]
//!                                          over-temperature guard (SoC > 90 C or FPGA > 75 C): warning + beep after
//!                                          3 periods, power off via the panel MCU after 11 (only with --power-off)
//!
//! `fpga-boot` and `cold-boot`: --force bypasses both loaded-module guards; a retry can hang the SoC.
//!
//! Options (defaults = the warm hand-over of 2026-11-02: CH1, 200 us/div, Auto depth, 1 V/div):
//!   --mask 1           channel mask (bit0 = CH1)          --app-mask 1   channel mask the stock system left behind
//!   --tdiv 200e-6      time/div in s                       --depth 0      memory depth (0 = Auto)
//!   --scale 1          V/div of all enabled channels       --offset 0     offset in V
//!   --level 1.5        trigger level in V                  --src 1        trigger source channel
//!   --slope rise|fall|either                               --normal       NORMAL sweep instead of AUTO
//!   --n 4000           points per channel to read          --at -2000     window start relative to the trigger point
//!   --all              read the whole record instead of --n/--at
//!   --play-last        read out with the play-last pulse of the warm hand-over instead of the pass-through + type-4 play
//!   --no-read          do all writes but not the c2h read (diagnostic: read with dd afterwards)
//!   --header           read with the 16-byte frame header (0x1010 bit31 = 0) like the stock live loop
//!   --keep-config      skip trigger, acquisition configuration and auto trigger (arm with what the stock system left behind)
//!   --compress 1       FPGA compression factor k of the whole record (0x1170, stock live display path; use with --all)
//!   --frames 1         frames per batch (one arm, one play, one read of all frames; capped at the frame ring)
//!   --average N        hardware average of N triggers (0x1004 bit3; reads the newest frame of the batch; not verified)
//!   --blocks 7         configuration blocks before the arm: bit0 trigger, bit1 acquisition, bit2 auto trigger
//!   --afe              program the AFEs of the enabled channels (scale/offset, bandwidth --bw OFF|S20M|AUTO|<MHz>)
//!   --la               logic analyser on (la.md, UNTESTED): LA set-up and a second, raw LA read after the analog one
//!                      (run writes the bytes of that last read, the LA words); --la-thr 1.4 threshold of both pods in V
//!                      (--la-alone: no analog channel shown, the analog rate is not halved)
//!   --raw-too          with --compress k > 1: read the newest frame once more without compression, to --out.raw
//!   --out /tmp/frame.bin
//!   oos-fpgad compress-check <raw.bin> <compressed.bin> --k K [--mask 1]
//!                                          offline (runs anywhere): which min/max pair layout explains the compressed
//!                                          read of the same record (oos_hal::dho::pairs, docs/fork/compress.md)
//! A channel set different from --app-mask switches the ADC mode; --afe and a mode switch read the calibration from
//! OOS_CAL_DIR (default /rigol/data).

mod boot_guard;
mod cmd;
#[cfg(any(target_os = "linux", test))]
mod pcie;

/// The commands `tool` runs (register, board and ADC access). A command missing here falls through to the usage text
/// without a word (unit round 9: `adc` did).
const TOOL_CMDS: [&str; 8] = ["status", "rd", "wr", "cold-boot", "fpga-boot", "pll", "gpio", "adc"];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");
    // the unit (docs/models.md): model from vendor.bin, standard limits plus /oos/limits.conf; read only
    let opt_arg = |k: &str| cmd::options::value(&args, k).flatten().map(str::to_owned);
    let cal_dir = std::env::var("OOS_CAL_DIR").unwrap_or_else(|_| "/rigol/data".into());
    let platform = oos_hal::model::Platform::detect(
        &cal_dir,
        &opt_arg("--limits").unwrap_or_else(|| oos_hal::model::LIMITS_CONF.into()),
        opt_arg("--model").as_deref(),
    );
    oos_hal::model::init(platform);
    if cmd == "model" {
        cmd::model::run();
        return;
    }
    // another board (docs/porting.md): its registers are not those of the DHO800/DHO900
    if let Some(m) = &oos_hal::model::platform().foreign {
        if !matches!(cmd, "help" | "--help" | "-h") {
            eprintln!(
                "oos-fpgad: this unit is a {}, another board: OOS does not drive its hardware (docs/porting.md)",
                m
            );
            std::process::exit(3);
        }
    }
    if cmd == "afg" && !oos_hal::model::platform().caps.has_gen() && !args.iter().any(|x| x == "--force") {
        eprintln!(
            "oos-fpgad: the {} has no generator (--force to drive the lines anyway)",
            oos_hal::model::platform().caps.name
        );
        std::process::exit(1);
    }
    let result = match cmd {
        "temp" | "beep" | "guard" => cmd::board(cmd, &args[2..]),
        "afg" => cmd::afg::run(&args[2..]),
        "la" => cmd::la::run(&args[2..]),
        "selfcal" => cmd::selfcal::run(&args[2..]),
        "roll" => cmd::roll::run(&args[2..]),
        "compress-check" => cmd::compress_check(&args[2..]),
        cmd if TOOL_CMDS.contains(&cmd) => cmd::tool(cmd, &args[2..]),
        "plan" => {
            cmd::plan::run(&args[2..]);
            Ok(())
        }
        "run" => {
            cmd::run::run(&args[2..]);
            Ok(())
        }
        _ => {
            for l in include_str!("main.rs").lines().take_while(|l| l.starts_with("//!")) {
                println!("{}", l.trim_start_matches("//!").trim_start_matches(' '));
            }
            Ok(())
        }
    };
    if let Err(e) = result {
        eprintln!("oos-fpgad: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_command_is_routed() {
        let src = include_str!("cmd/mod.rs");
        let body = &src[src.find("fn tool(cmd: &str").unwrap()..];
        let body = &body[..body.find("\n}\n").unwrap()];
        let arms: Vec<&str> =
            body.lines().filter_map(|l| l.strip_prefix("        \"")?.split_once("\" =>").map(|(c, _)| c)).collect();
        assert!(arms.len() >= 6, "{:?}", arms);
        for c in arms {
            assert!(TOOL_CMDS.contains(&c), "tool arm {} missing from TOOL_CMDS", c);
        }
    }
}
