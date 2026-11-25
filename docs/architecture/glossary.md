# Domain glossary

Use these nouns in types and public APIs. The `naming` gate reads these tables; lowercase words match whole
snake_case or CamelCase components, including plurals. Forbidden type words are legacy synonyms, frozen in
`xtask/data/<crate>/naming.txt`. A packet remains a transport packet, not an acquisition batch.
Review checks whether an allowed term actually describes the concept.

| Canonical | Forbidden type words | Meaning |
|---|---|---|
| record | capture, recording | One retained acquisition with its own metadata. |
| sweep | scan | One trigger-aligned traversal of samples. |
| batch | bundle | A publication containing multiple sweeps. |
| deposit | deposition | Sample contributions to the picture accumulator. |
| picture | phosphor | Accumulated intensities, weights and window. |
| waveform | wave | Signal values represented for analysis or export. |
| channel | chan | A physical or logical signal input. |
| trace | curve | A displayed signal path. |
| acquisition | - | Obtaining samples from a source. |
| trigger | - | The condition aligning a sweep. |
| sample | - | One measured or synthesized value. |
| frame | - | One published acquisition envelope. |
| command | - | A requested settings change or action. |
| query | - | A read of application state. |
| setting | - | A configurable domain value. |
| request | - | An owned operation crossing a port. |
| response | - | The result of a request. |
| message | - | A complete protocol unit. |
| event | - | A reported state transition. |
| connection | - | A transport session. |
| socket | - | A transport endpoint. |
| pixel | - | One display location. |
| file | - | A stored artifact. |
| register | - | A hardware control or observation word. |

## Abbreviations

Acronyms and words of at most three letters in declaration names must appear below. Longer contractions are
listed explicitly in the last table; spelling and meaning of other long words are reviewed. Standard Rust names
and common short English words are included so the check does not demand invented replacements.

| Abbreviation | Meaning |
|---|---|
| adc, acq, dac, dsp, dma, fpga, gpio, hal | Established acquisition and hardware terms. |
| dho, dho804, dho800, dho900, scu, spu, tpu, vpu, gpu | Hardware families and processing units. |
| api, ascii, bcd, can, cphy, crc, csv, dcc, dmx, dmx512, dns, edid, elf | Protocol and file formats. |
| esr, fifo, hid, hislip, html, http, https, i2c, i2s, i3c, ip, ipv4, ipv6 | Protocol terms. |
| jpeg, json, la, lan, lcd, lin, lsb, mac, mcp, mdns, msb, ntp, pci, pcie | Protocol and hardware terms. |
| pid, png, pps, qspi, ram, rgb, rpc, scpi, simd, sntp, spdif, spi, ssh | Protocol and representation terms. |
| tcp, tmc, ttl, uart, udp, ui, usb, usbtmc, utf8, uuid, vxi, vxi11, wlan | Protocol and interface terms. |
| arc, box, cow, dyn, err, fmt, fn, into, io, iter, mut, ok, ord, os, pin, rc, ref, rhs, str, vec | Standard Rust vocabulary. |
| sin, cos, tan, asin, acos, atan, atan2, fft, ifft | Standard mathematical functions and transforms. |
| i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, f32, f64 | Rust numeric primitives. |
| id, ids, hz, khz, mhz, ghz, ns, us, ms, db, dbm, rms | Identifiers and units. |
| add, all, and, any, arm, bus, doc, git, got, now, own, put, url | Common English, Rust documentation and platform terms. |
| a, an, as, at, be, bit, by, do, end, eof, eq, for, from, get, has, if, in, is, key, len, let | Short English and API words. |
| log, low, map, max, min, new, no, not, of, off, on, one, or, out, pop, raw, row, run, set | Short English and API words. |
| sum, tag, the, to, top, try, two, up, use, via, was, with, x, y, z | Short English and coordinate words. |

## Contractions to spell out

| Contraction | Replacement |
|---|---|
| cfg, conf, config | configuration |
| ctx, ctxt | context |
| buf | buffer |
| cmd | command |
| msg | message |
| req | request |
| resp | response |
| opts | options |
| params | parameters |
| init | initialize |
| recv | receive |
| temp, tmp | temporary or temperature, according to meaning |
| wfm, wav | waveform |
