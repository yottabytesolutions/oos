// SPDX-License-Identifier: GPL-2.0-or-later

//! The sidecar records the armed TPU settings, which can differ from the software trigger.

use super::{cast, finite_str};
use oos_hal::dho::{seq::Trig, serialtrig, tpu, uarttrig};

macro_rules! variants {
    ($parse:ident, $module:ident :: $ty:ident => $($variant:ident),+ $(,)?) => {
        fn $parse(s: &str) -> Result<$module::$ty, String> {
            match s {
                $(stringify!($variant) => Ok($module::$ty::$variant),)+
                _ => Err(format!("invalid trigger {}", stringify!($ty))),
            }
        }
    };
}

variants!(pol, tpu::Polarity => Pos, Neg, Any);
variants!(when, tpu::When => Gt, Lt, In, Out, None);
variants!(event, tpu::WindowEvent => Enter, Exit, Time);
variants!(edge, tpu::Edge => Rising, Falling);
variants!(pat, tpu::Pat => H, L, X, Rise, Fall);
variants!(standard, tpu::VideoStd => Ntsc, Pal);
variants!(sync, tpu::VideoSync => AnyLine, Line, OddField, EvenField);
variants!(rel, uarttrig::Relation => Eq, Ne, Gt, Lt, In, Out);
variants!(error, uarttrig::ErrKind => Parity, Framing, Either);
variants!(parity, uarttrig::Parity => None, Odd, Even, Unchecked);
variants!(stop, uarttrig::Stop => One, Two, OneHalf);
variants!(i2c_cond, serialtrig::I2cCond => Start, Stop, RepeatedStart, MissingAck, Address, Data, AddressData);
variants!(dir, serialtrig::Dir => Write, Read, Either);

pub(super) fn parse(input: &str) -> Result<Trig, String> {
    // Debug names make captures readable. Strip punctuation and named fields, preserving the ordered values.
    let tokens = input.replace(':', " : ");
    let mut tokens = tokens
        .split(|ch: char| ch.is_ascii_whitespace() || "{}[](),".contains(ch))
        .filter(|token| !token.is_empty())
        .peekable();
    let kind = tokens.next().ok_or("missing trigger type")?;
    let mut fields = Vec::new();
    while let Some(token) = tokens.next() {
        if tokens.next_if_eq(&":").is_none() {
            fields.push(token);
        }
    }
    decode(kind, &fields)
}

fn number<T: std::str::FromStr>(s: &str) -> Result<T, String> {
    cast("trigger setting", s)
}

fn float(s: &str) -> Result<f64, String> {
    finite_str("trigger setting", s)
}

fn array<T, const N: usize>(fields: &[&str], parse: impl Fn(&str) -> Result<T, String>) -> Result<[T; N], String> {
    fields
        .iter()
        .map(|s| parse(s))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "invalid trigger array".into())
}

fn pattern(fields: &[&str]) -> Result<([tpu::Pat; 4], [tpu::Pat; 16]), String> {
    let (pats, la) = fields.split_at(4);
    Ok((array(pats, pat)?, array(la, pat)?))
}

fn decode(kind: &str, fields: &[&str]) -> Result<Trig, String> {
    Ok(match (kind, fields) {
        ("Edge", []) => Trig::Edge,
        ("Pulse", [p, w, lo, hi]) => Trig::Pulse { pol: pol(p)?, when: when(w)?, lower: float(lo)?, upper: float(hi)? },
        ("Slope", [w, a, b, lo]) => Trig::Slope { when: when(w)?, lower: float(a)?, upper: float(b)?, lo: float(lo)? },
        ("Runt", [p, w, a, b, lo]) => {
            Trig::Runt { pol: pol(p)?, when: when(w)?, lower: float(a)?, upper: float(b)?, lo: float(lo)? }
        }
        ("Window", [e, time, lo]) => Trig::Window { event: event(e)?, time: float(time)?, lo: float(lo)? },
        ("Timeout", [time]) => Trig::Timeout { time: float(time)? },
        ("Delay", [srcb, eb, w, lower, upper, levelb]) => Trig::Delay {
            srcb: number(srcb)?,
            eb: edge(eb)?,
            when: when(w)?,
            lower: float(lower)?,
            upper: float(upper)?,
            levelb: float(levelb)?,
        },
        ("Pattern", fields) if fields.len() == 24 => {
            let (pats, la) = pattern(&fields[..20])?;
            Trig::Pattern { pats, la, levels: array(&fields[20..], float)? }
        }
        ("Duration", fields) if fields.len() == 27 => {
            let (pats, la) = pattern(&fields[..20])?;
            Trig::Duration {
                pats,
                la,
                when: when(fields[20])?,
                lower: float(fields[21])?,
                upper: float(fields[22])?,
                levels: array(&fields[23..], float)?,
            }
        }
        ("SetupHold", [clk, e, setup, hold, levelc]) => Trig::SetupHold {
            clk: number(clk)?,
            clk_edge: edge(e)?,
            setup: float(setup)?,
            hold: float(hold)?,
            levelc: float(levelc)?,
        },
        ("Nth", [count, idle]) => Trig::Nth { count: number(count)?, idle: float(idle)? },
        ("Video", [std, s, line]) => Trig::Video { std: standard(std)?, sync: sync(s)?, line: number(line)? },
        ("Uart", ["Uart", parts @ ..]) => Trig::Uart(uart(parts)?),
        ("Spi", ["Spi", parts @ ..]) => Trig::Spi(spi(parts)?),
        ("I2c", ["I2c", parts @ ..]) => Trig::I2c(i2c(parts)?),
        _ => return Err(format!("invalid trigger settings for {kind}")),
    })
}

fn uart(fields: &[&str]) -> Result<uarttrig::Uart, String> {
    use uarttrig::Cond;
    let [baud, bits, p, s, cond @ .., invert] = fields else { return Err("invalid UART trigger".into()) };
    let cond = match cond {
        ["Start"] => Cond::Start,
        ["Data", r, min, max] => Cond::Data(rel(r)?, number(min)?, number(max)?),
        ["Error", e] => Cond::Error(error(e)?),
        _ => return Err("invalid UART condition".into()),
    };
    Ok(uarttrig::Uart {
        baud: float(baud)?,
        bits: number(bits)?,
        parity: parity(p)?,
        stop: stop(s)?,
        cond,
        invert: number(invert)?,
    })
}

fn spi(fields: &[&str]) -> Result<serialtrig::Spi, String> {
    let [clk, data, cs, falling, high, timeout @ .., width, r, min, max, mask] = fields else {
        return Err("invalid SPI trigger".into());
    };
    let timeout = match timeout {
        ["None"] => None,
        ["Some", t] => Some(float(t)?),
        _ => return Err("invalid SPI timeout".into()),
    };
    Ok(serialtrig::Spi {
        clk: number(clk)?,
        data: number(data)?,
        cs: number(cs)?,
        clk_falling: number(falling)?,
        cs_high: number(high)?,
        timeout,
        width: number(width)?,
        rel: rel(r)?,
        min: number(min)?,
        max: number(max)?,
        mask: number(mask)?,
    })
}

fn i2c(fields: &[&str]) -> Result<serialtrig::I2c, String> {
    let [scl, sda, cond, d, ten_bit, ar, amin, amax, amask, dr, bytes, dmin, dmax, dmask] = fields else {
        return Err("invalid I2C trigger".into());
    };
    Ok(serialtrig::I2c {
        scl: number(scl)?,
        sda: number(sda)?,
        cond: i2c_cond(cond)?,
        dir: dir(d)?,
        ten_bit: number(ten_bit)?,
        addr_rel: rel(ar)?,
        addr: (number(amin)?, number(amax)?, number(amask)?),
        data_rel: rel(dr)?,
        bytes: number(bytes)?,
        data: (number(dmin)?, number(dmax)?, number(dmask)?),
    })
}

#[test]
fn serial_trigger_settings_round_trip() {
    use uarttrig::{Cond, Parity, Relation, Stop};
    let triggers = [
        Trig::Uart(uarttrig::Uart {
            baud: 115_200.0,
            bits: 8,
            parity: Parity::Even,
            stop: Stop::Two,
            cond: Cond::Data(Relation::In, 4, 20),
            invert: true,
        }),
        Trig::Spi(serialtrig::Spi {
            clk: 1,
            data: 2,
            cs: 3,
            clk_falling: true,
            cs_high: false,
            timeout: Some(4e-6),
            width: 24,
            rel: Relation::Eq,
            min: 20,
            max: 20,
            mask: 0x00ff_ffff,
        }),
        Trig::I2c(serialtrig::I2c {
            scl: 2,
            sda: 4,
            cond: serialtrig::I2cCond::AddressData,
            dir: serialtrig::Dir::Read,
            ten_bit: true,
            addr_rel: Relation::In,
            addr: (128, 200, 1023),
            data_rel: Relation::Ne,
            bytes: 3,
            data: (34, 100, 0x00ff_ffff),
        }),
    ];
    for trigger in triggers {
        assert_eq!(parse(&format!("{trigger:?}")).unwrap(), trigger);
    }
}
