// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::dho::cores::Cores;
use oos_hal::dho::seq;
use oos_hal::dho::seq::CodeMap;
use std::fmt::Write;

pub(super) struct Lane<'a> {
    bytes: &'a [u8],
    ratio: usize,
    word: usize,
}

impl Lane<'_> {
    pub(super) fn len(&self) -> usize {
        (self.bytes.len() / 2 - self.word).div_ceil(self.ratio)
    }

    pub(super) fn sample(&self, i: usize) -> u16 {
        let j = 2 * (i * self.ratio + self.word);
        u16::from_le_bytes([self.bytes[j], self.bytes[j + 1]])
    }
}

pub(super) fn visit_lanes(bytes: &[u8], header: bool, map: &CodeMap, mut visit: impl FnMut(usize, Lane<'_>)) {
    let bytes = &bytes[if header { 16 } else { 0 }..];
    for (c, word) in map.word.into_iter().enumerate() {
        if let Some(word) = word.filter(|&w| w < bytes.len() / 2) {
            visit(c, Lane { bytes, ratio: map.ratio, word });
        }
    }
}

const TICK_MASK: u64 = (1u64 << 48) - 1;

fn timestamp(words: [u16; 8]) -> u64 {
    u64::from(words[5]) << 32 | u64::from(words[6]) << 16 | u64::from(words[7])
}

fn phase(fixture: &Fixture) {
    let (s, _) = fixture.settings();
    if !s.header || !crate::board::dho::matching::eligible(s) {
        eprintln!("{}: phase report requires headers and uncompressed Normal samples", fixture.name());
        return;
    }
    let map = seq::code_map(s.mask, s.ch.map(|c| (c.scale, c.offset)));
    let mut cores = Cores::default();
    let mut patterns = fixture.patterns();
    let seeds = patterns.map(|p| p.is_some());
    let first_good = fixture.expected_route().map_or(0, |(_, range)| range.start);
    for bytes in fixture.records().skip(first_good) {
        visit_lanes(bytes, true, &map, |c, lane| {
            if patterns[c].is_none() {
                patterns[c] = cores.fixture_seed(lane.len(), |i| lane.sample(i));
            }
        });
        if map.word.iter().zip(patterns).all(|(word, pattern)| word.is_none() || pattern.is_some()) {
            break;
        }
    }

    let mut previous = None;
    for (index, bytes) in fixture.records().enumerate() {
        let words = super::super::words(bytes);
        let ticks = timestamp(words);
        let delta =
            previous.map_or_else(|| "null".to_owned(), |prev| ((ticks.wrapping_sub(prev)) & TICK_MASK).to_string());
        previous = Some(ticks);
        let mut channels = String::new();
        visit_lanes(bytes, true, &map, |c, lane| {
            if !channels.is_empty() {
                channels.push(',');
            }
            let _ = write!(
                channels,
                "{{\"channel\":{},\"pattern\":\"{}\"",
                c + 1,
                if seeds[c] { "captured" } else { "fixture_seed" }
            );
            match patterns[c].and_then(|pattern| cores.fixture_fit(pattern, lane.len(), |i| lane.sample(i))) {
                Some((turn, residual, reference)) => {
                    let _ = write!(channels, ",\"turn\":{turn},\"residual_codes_squared\":{residual},\"reference_codes_squared\":{reference},\"fit\":{}", residual < reference);
                }
                None => channels.push_str(",\"turn\":null,\"residual_codes_squared\":null"),
            }
            channels.push('}');
        });
        println!(
            "{{\"fixture\":{},\"record\":{index},\"frame_number\":{},\"timestamp_ticks\":{ticks},\"delta_ticks\":{delta},\"tick_ns\":0.8,\"timestamp_mod8\":{},\"words\":{words:?},\"channels\":[{channels}]}}",
            crate::json::J::s(fixture.name()).to_string(), words[4], ticks & 7,
        );
    }
}

#[test]
#[ignore = "X7 report for captured Normal batches with headers"]
fn header_phase_report() {
    for_each_fixture(&mut phase);
}

#[test]
fn fixture_corrections_agree_with_full_record_phase() {
    let (mut exact, mut decided, mut batches) = (0, 0, 0);
    for_each_fixture(&mut |fixture| {
        let Some((_, range)) = fixture.expected_route() else { return };
        let (bytes, (fb, _, _), _) = fixture.source();
        let (same, count) = crate::board::dho::matching::replay_corrections(
            fixture.settings(),
            &bytes,
            fb,
            range,
            &mut fixture.cores(),
        );
        exact += same;
        decided += count;
        batches += 1;
    });
    eprintln!("core replay: {exact}/{decided} records with identical oracle corrections across {batches} batches");
    assert!(exact * 100 >= decided * 95, "{exact}/{decided} exact corrections");
}

#[test]
fn header_timestamp_is_most_significant_word_first_and_wraps_at_48_bits() {
    let words = super::super::words(&[5, 250, 0, 0, 123, 0, 0, 0, 4, 0, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xff]);
    assert_eq!(words[4], 4);
    assert_eq!(timestamp(words), TICK_MASK - 1);
    assert_eq!(1u64.wrapping_sub(timestamp(words)) & TICK_MASK, 3);
}
