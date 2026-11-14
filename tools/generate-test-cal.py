#!/usr/bin/env python3
"""Regenerate hal/testdata/synthetic and model expectations without unit data.

Only standard-library modules and the in-tree ADC/AFE reference models are used.
All values below are invented. Never install these files on hardware.
Run from any directory: python3 tools/generate-test-cal.py
"""

import importlib.util
import os
import tempfile
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "hal/testdata/synthetic"


def container(payload):
    # Fixed 2000-01-01 00:00:00 stamp, version zero, both CRCs checked by Rust.
    header = struct.pack("<5I", 0, 1 << 24, 2000 << 8 | 1,
                         len(payload), zlib.crc32(payload))
    return struct.pack("<2I", zlib.crc32(header), 20) + header + payload


def save(name, payload):
    (DEST / name).write_bytes(container(payload))


def ladder():
    keys = []
    for decade in range(5):
        base = 100_000 * 10 ** decade
        keys += [base + k * base // 50 for k in range(50)]
        keys += [2 * base + k * base // 20 for k in range(60)]
        keys += [5 * base + k * base // 10 for k in range(50)]
    return keys + [10_000_000_000]


def generate():
    DEST.mkdir(parents=True, exist_ok=True)
    adc = bytearray(0x738)
    for mode in range(11):
        for core in range(8):
            for offset, value in [(0, core - 4 + mode), (0x10, 4 - core),
                                  (0x20, core - 3), (0x30, 3 - core)]:
                struct.pack_into("<h", adc, mode * 0x68 + offset + core * 2, value)
        for i, value in enumerate([2, 3, 4, 5, 17, 19]):
            struct.pack_into("<h", adc, mode * 0x68 + 0x40 + i * 2, value)
    for ch in range(4):
        for slot in range(16):
            struct.pack_into("<i", adc, 0x478 + ch * 64 + slot * 4, (ch + slot) % 4)
    save("cal_adc.hex", adc)
    save("cal_ddr.hex", struct.pack("<6Q", 0x21, 0x345, 0x678, 0x789, 0x123, 0))
    save("cal_lsb.hex", struct.pack("<16d", *[0.001 * (ig + 1) for ch in range(4) for ig in range(4)]))
    save("cal_afe_zero.hex", struct.pack("<80i", *[20 + i % 7 for i in range(80)]))
    save("cal_afe_bandwidth.hex", struct.pack("<80i", *[40 + i % 11 for i in range(80)]))
    keys = ladder()
    vertical = bytearray()
    for group in range(4):
        for ch in range(4):
            for i, key in enumerate(keys):
                # 81 distinct, valid gain tuples. Each ten-row band uses the next
                # band's boundary plus half a nV, as rebuild_scales requires >.
                band = i // 10
                gains = [band % 4, 2, band // 4 % 4, band // 16 % 4,
                         band // 64, ch % 4, group % 4, 0]
                scale = keys[min(band * 10 + 9, 800)] + 0.5
                vertical += struct.pack("<q8idii", key, *gains, scale / key,
                                        32000 + ch * 10 + group, 0)
    save("cal_la.hex", struct.pack("<2I", 8500, 8520))
    afg = bytearray(0x18 + 40 * 0x968)
    for row in range(40):
        base = 0x18 + row * 0x968
        struct.pack_into("<i", afg, base, 1)
        lo, hi = (-10**15, 10**15) if not 30 <= row < 34 else (10**15, -10**15)
        struct.pack_into("<q", afg, base + 8, lo)
        struct.pack_into("<q", afg, base + 0x328, hi)
        k, b = (1.0, 1_000_000.0) if row < 30 else (0.0, 1.0)
        struct.pack_into("<f", afg, base + 0x648, k)
        struct.pack_into("<f", afg, base + 0x7d8, b)
    save("cal_afg.hex", afg)
    return container(vertical)


def model(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tools" / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def expectations(cal_dir):
    # AFE's reference model accepts an environment directory; explicitly replace
    # that input before importing it so no private default is ever read.
    os.environ["OOS_CAL_DIR"] = str(cal_dir)
    adc = model("adcmodel")
    chip = adc.Adc(adc.CalAdc(str(cal_dir)), False)
    out = []

    def writes():
        out.extend(f"{pre} {reg:#x} {val:#x} {post}" for reg, val, pre, post, _ in chip.w)
        chip.w = []

    out.append("== boot")
    chip.adc_parameter(0)
    writes()
    for mask in [3, 15, 12, 1]:
        out.append(f"== {mask:#x}")
        chip.channel_mode(adc.MODE_OF_MASK[mask])
        writes()
        out.append("-- wpu " + " ".join(f"{ch}:{v:#x}" for ch, v in adc.wpu_gain(mask, [1] * 4)))
    out.append("== onoff")
    chip.adc_onoff()
    writes()
    (DEST.parent / "adc_expected.txt").write_text("\n".join(out) + "\n")
    afe = model("afesim")
    out = []
    for ch, scale, offset, bw in [(1, 1.0, 0.0, "OFF"), (2, 0.1, 0.02, "S20M"),
                                  (3, 0.002, -0.001, "Auto"), (4, 5.0, 1.0, "350M"),
                                  (1, 0.5, -0.1, "100M"), (2, 0.01, 0.0, "300M")]:
        out.append(f"== {ch} {scale} {offset} {bw}")
        log = []
        front = afe.Afe(ch - 1, log)
        value = afe.BW_VALUE[bw]
        filt = afe.bw_filter(value)
        group = afe.bw_variant(value, afe.bw_auto_c(1.25e9))
        out.append("filter %d %d %d group %d" % (*filt, group))
        front.hz_filter(*filt)
        rec = afe.set_scale(front, int(scale * 1e9), group, fixed_trim=True)
        code = afe.set_offset(front, rec, int(offset * 1e9))
        for op in log:
            if isinstance(op, tuple):
                out.append("w %d %#04x %#06x" % (op[0] + 1, op[1], op[2]))
            else:
                out.append("gear %#04x" % ((ch - 1) | 0x10))
        out.append(f"dac {code} gain {rec[9]:.5f}")
    (DEST.parent / "afe_expected.txt").write_text("\n".join(out) + "\n")


if __name__ == "__main__":
    vertical = generate()
    with tempfile.TemporaryDirectory(prefix="oos-test-cal-") as tmp:
        cal_dir = Path(tmp)
        for source in DEST.glob("*.hex"):
            (cal_dir / source.name).write_bytes(source.read_bytes())
        (cal_dir / "cal_vertical.hex").write_bytes(vertical)
        expectations(cal_dir)
