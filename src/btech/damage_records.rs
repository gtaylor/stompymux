//! Ordered compact damage records shared by inspection and administrative replacement planning.
use anyhow::{Context, Result, ensure};
use std::{fmt, str::FromStr};

/// A format-level record; anatomy, installed equipment and resulting material need separate validation.
/// Signed losses retain the text contract without implying that an increased material value is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageRecord {
    /// Loss from original armor on the indicated face.
    Armor { section: u8, rear: bool, loss: i32 },
    /// Loss from original internal structure.
    Internal { section: u8, loss: i32 },
    /// Destroy one installed critical slot.
    Critical { section: u8, slot: u8 },
    /// Rounds spent from the full capacity of an ammunition bin.
    Ammunition { section: u8, slot: u8, spent: i32 },
    /// Numeric temporary failure on an installed critical slot.
    Failure { section: u8, slot: u8, failure: i32 },
}

impl fmt::Display for DamageRecord {
    /// Use the same canonical record spelling as the inspection field.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Armor {
                section,
                rear,
                loss,
            } => write!(out, "{}:{section}/{loss}", if rear { "A(R)" } else { "A" }),
            Self::Internal { section, loss } => write!(out, "I:{section}/{loss}"),
            Self::Critical { section, slot } => write!(out, "C:{section}/{slot}"),
            Self::Ammunition {
                section,
                slot,
                spent,
            } => write!(out, "R:{section}/{slot}({spent})"),
            Self::Failure {
                section,
                slot,
                failure,
            } => write!(out, "G:{section}/{slot}({failure})"),
        }
    }
}

/// Decode a format index without accepting out-of-range section or critical identities.
fn index(value: &str, limit: u8, name: &str) -> Result<u8> {
    let value = value
        .parse::<u8>()
        .with_context(|| format!("Invalid damage {name}"))?;
    ensure!(value < limit, "Damage {name} is out of range");
    Ok(value)
}

impl FromStr for DamageRecord {
    type Err = anyhow::Error;

    /// Parse exactly one record; unrecognized keywords and trailing input are errors.
    fn from_str(value: &str) -> Result<Self> {
        let (kind, values) = value
            .split_once(':')
            .context("Damage record needs a type and colon")?;
        let (section, values) = values
            .split_once('/')
            .context("Damage record needs a section and slash")?;
        let section = index(section, 8, "section")?;
        match kind {
            "A" | "A(R)" => Ok(Self::Armor {
                section,
                rear: kind == "A(R)",
                loss: values.parse().context("Invalid armor loss")?,
            }),
            "I" => Ok(Self::Internal {
                section,
                loss: values.parse().context("Invalid internal loss")?,
            }),
            "C" => Ok(Self::Critical {
                section,
                slot: index(values, 12, "slot")?,
            }),
            "R" | "G" => {
                let (slot, amount) = values
                    .split_once('(')
                    .context("Damage record needs a parenthesized value")?;
                let amount = amount
                    .strip_suffix(')')
                    .context("Damage record needs a closing parenthesis")?;
                let slot = index(slot, 12, "slot")?;
                let amount = amount
                    .parse::<i32>()
                    .context("Invalid damage record value")?;
                Ok(if kind == "R" {
                    Self::Ammunition {
                        section,
                        slot,
                        spent: amount,
                    }
                } else {
                    Self::Failure {
                        section,
                        slot,
                        failure: amount,
                    }
                })
            }
            _ => anyhow::bail!("Unknown damage record type {kind}"),
        }
    }
}

/// Parse the full replacement description without sorting or discarding repeated assignments.
/// An empty description is valid; the eventual replacement owner decides what omitted records restore.
/// This function does not mutate units or apply damage, repairs, failures or ammunition changes.
pub fn parse_damage_field(value: &str) -> Result<Vec<DamageRecord>> {
    value
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .filter(|record| !record.is_empty())
        .enumerate()
        .map(|(index, record)| {
            record
                .parse()
                .with_context(|| format!("Damage record {}: {record}", index + 1))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty replacement and repeated assignments retain their distinct meaning.
    #[test]
    fn ordered_records_round_trip_without_collapsing_assignments() {
        assert!(parse_damage_field(" ,\n\t ").unwrap().is_empty());
        let text = "A:0/8,A(R):2/3,I:7/1,C:4/11,R:3/9(12),G:0/4(7),A:0/2";
        let records = parse_damage_field(text).unwrap();
        assert_eq!(records.len(), 7);
        assert_eq!(
            records
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            text
        );
        assert_eq!(
            parse_damage_field(&text.replace(',', " \t\n")).unwrap(),
            records
        );
        assert_eq!(
            records[0],
            DamageRecord::Armor {
                section: 0,
                rear: false,
                loss: 8
            }
        );
        assert_eq!(
            records[6],
            DamageRecord::Armor {
                section: 0,
                rear: false,
                loss: 2
            }
        );
    }

    /// Syntax preserves bounded signed values; material admission belongs to the unit owner.
    #[test]
    fn signed_values_round_trip_at_integer_boundaries() {
        for value in [i32::MIN, -1, 0, 1, i32::MAX] {
            for text in [
                format!("A:0/{value}"),
                format!("I:7/{value}"),
                format!("R:3/11({value})"),
                format!("G:7/0({value})"),
            ] {
                let record: DamageRecord = text.parse().unwrap();
                assert_eq!(record.to_string(), text);
            }
        }
    }

    /// A malformed later token rejects the complete description rather than silently dropping it.
    #[test]
    fn invalid_syntax_and_indices_fail_as_a_whole() {
        for record in [
            "A",
            "a:0/1",
            "X:0/1",
            "A:8/1",
            "A:-1/1",
            "C:0/12",
            "C:0/-1",
            "C:0/1(2)",
            "A:0/1/2",
            "A:0/1x",
            "R:0/1",
            "R:0/1()",
            "R:0/1(2)junk",
            "R:0/1((2))",
            "G:0/1(2147483648)",
            "I:0/-2147483649",
            "A:0/NaN",
            "I:0/1.5",
        ] {
            assert!(record.parse::<DamageRecord>().is_err(), "{record}");
            assert!(
                parse_damage_field(&format!("A:0/1,{record},I:0/1")).is_err(),
                "{record}"
            );
        }
    }
}
