//! Shared bounded bitvector input and presentation for administrative fields.
use anyhow::{Context, Result, bail, ensure};

/// Parse a signed integer or a bounded letter bitvector, constructed from zero.
pub(super) fn parse(value: &str) -> Result<i64> {
    if value == "-" {
        return Ok(0);
    }
    if let Ok(number) = value.parse::<i32>() {
        return Ok(i64::from(number));
    }
    ensure!(!value.is_empty(), "Empty bitvector");
    let mut bits = 0_u32;
    let mut letters = value.bytes();
    while let Some(mut letter) = letters.next() {
        let clear = letter == b'!';
        if clear {
            letter = letters.next().context("Missing bit after !")?;
        }
        let bit = match letter {
            b'a'..=b'z' => letter - b'a',
            b'A'..=b'F' => 26 + letter - b'A',
            _ => bail!("Bitvector letters must be a-z or A-F"),
        };
        if clear {
            bits &= !(1_u32 << bit);
        } else {
            bits |= 1_u32 << bit;
        }
    }
    Ok(i64::from(bits as i32))
}

/// Display every supported bit without signed shifting or hiding the highest bit.
pub(super) fn format(value: i64) -> String {
    let value = value as u32;
    if value == 0 {
        return "-".into();
    }
    (0..32)
        .filter(|bit| value & (1 << bit) != 0)
        .map(|bit| {
            char::from(if bit < 26 {
                b'a' + bit
            } else {
                b'A' + bit - 26
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every displayed mask, including the empty marker and sign bit, is reusable as input.
    #[test]
    fn displayed_masks_can_be_written_back() {
        for value in [0, 1, i64::from(i32::MIN), i64::from(i32::MAX), -1] {
            assert_eq!(parse(&format(value)).unwrap(), value);
        }
    }
}
