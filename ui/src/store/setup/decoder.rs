// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) const DECODER_SELECTION: &[Key] = keys! { m, _i, v;
    "dec_sel" => value(m.dec_sel, p_idx(v, DECODERS)?);
};
pub(super) const DECODER: &[Key] = keys! { m, _i, v;
    "dec_on" => value(m.dec_on, p_bool(v)?);
    "dec_src" => value(m.dec_src, p_decode_source(v)?);
    "dec_baud" => value(m.dec_baud, p_idx(v, crate::decode::BAUDS.len())?);
    "dec_canrate" => value(m.dec_canrate, p_idx(v, crate::decode::CAN_RATES.len())?);
    "dec_canh" => value(m.dec_canh, p_bool(v)?);
    "dec_fdrate" => value(m.dec_fdrate, p_idx(v, crate::decode::FD_RATES.len())?);
    "dec_arate" => value(m.dec_arate, p_idx(v, crate::decode::ARINC_RATES.len())?);
    "dec_tick" => value(m.dec_tick, p_idx(v, crate::decode::SENT_TICKS.len())?);
    "dec_nibbles" => value(m.dec_nibbles, p_u32(v)?.clamp(1, 6));
    "dec_uspeed" => value(m.dec_uspeed, p_idx(v, crate::decode::USB_SPEEDS.len())?);
    "dec_cphy_rate" => value(m.dec_cphy_rate, crate::decode::parse_cphy_rate(v)?);
    "dec_src4" => custom(
        Some(m.dec_src4.map_or("none".into(), |c| (c + 1).to_string())),
        m.dec_src4 = p_decode_optional_one_based(v)?
    );
    "dec_diff" => custom(
        Some(m.dec_diff.map_or("none".into(), |c| (c + 1).to_string())),
        m.dec_diff = p_decode_optional_one_based(v)?
    );
    "trig_bus" => custom(
        Some(m.trig_bus.name().to_string()),
        {
            m.trig_bus =
                *BUS_WHATS.iter().find(|b| b.name() == v.trim()).ok_or_else(|| format!("unknown bus match {}", v))?;
        }
    );
    "trig_bus_val" => value(m.trig_bus_val, v.trim().parse().map_err(|e| format!("{}", e))?);
    "ser.rel" => named(m.ser.rel, p_enum(v, &RELS)?);
    "ser.max" => value(m.ser.max, v.trim().parse().map_err(|e| format!("ser.max: {}", e))?);
    "ser.mask" => value(m.ser.mask, v.trim().parse().map_err(|e| format!("ser.mask: {}", e))?);
    "ser.uart_inv" => value(m.ser.uart_inv, p_bool(v)?);
    "ser.uart_stop" => named(m.ser.uart_stop, p_enum(v, &STOP_BITS)?);
    "ser.uart_err" => named(m.ser.uart_err, p_enum(v, &UART_ERRS)?);
    "ser.spi_cs_high" => value(m.ser.spi_cs_high, p_bool(v)?);
    "ser.spi_timeout" => value(m.ser.spi_timeout, p_f32(v)?.max(0.0));
    "ser.addr_rel" => named(m.ser.addr_rel, p_enum(v, &RELS)?);
    "ser.addr_max" => value(m.ser.addr_max, p_u32(v)?.min(0x3ff));
    "ser.dir" => named(m.ser.dir, p_enum(v, &I2C_DIRS)?);
    "ser.ten_bit" => value(m.ser.ten_bit, p_bool(v)?);
    "ser.bytes" => value(m.ser.bytes, p_u32(v)?.min(5) as u8);
    "dec_thr" => value(m.dec_thr, p_f32(v)?);
    "dec_ascii" => value(m.dec_ascii, p_bool(v)?);
    "dec_proto" => custom(
        Some(m.dec_proto.key()),
        m.dec_proto = Proto::from_key(v.trim()).ok_or_else(|| format!("unknown protocol: {}", v.trim()))?
    );
    "dec_opt" => custom(
        Some(list(&m.dec_opt)),
        m.dec_opt.copy_from_slice(&p_list(v, crate::protocols::OPTS, |x| x.parse::<u8>().map_err(|e| e.to_string()))?)
    );
    "dec_src2" => value(m.dec_src2, p_decode_source(v)?);
    "dec_cs" => custom(
        Some(m.dec_cs.map_or("none".into(), |c| c.to_string())),
        m.dec_cs = if v == "none" { None } else { Some(p_decode_source(v)?) }
    );
    "dec_mode" => value(m.dec_mode, p_idx(v, 4)? as u8);
    "dec_bits" => value(m.dec_bits, p_u32(v)?.clamp(4, 32));
    "dec_parity" => named(m.dec_parity, p_enum(v, &PARITIES)?);
};
pub(super) const BUS: &[Key] = keys! { m, _i, v;
    "bus_on" => value(m.bus.on, p_bool(v)?);
    "bus_label" => custom(Some(m.bus.label.clone()), m.bus.label = crate::model::bus_label(v));
    "bus_pos" => value(m.bus.pos, p_f32(v)?.clamp(BUS_POS_MIN, BUS_POS_MAX));
    "bus_kind" => named(m.bus.kind, p_enum(v, &[BusKind::I2c])?);
    "bus_clk" => custom(Some(m.bus.clk.token()), m.bus.clk = WaveId::parse(v)?);
    "bus_data" => custom(Some(m.bus.data.token()), m.bus.data = WaveId::parse(v)?);
    "bus_clk_thr" => value(m.bus.clk_thr, p_f32(v)?.clamp(-40.0, 40.0));
    "bus_data_thr" => value(m.bus.data_thr, p_f32(v)?.clamp(-40.0, 40.0));
    "bus_rw" => value(m.bus.addr_rw, p_bool(v)?);
};
