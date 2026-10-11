//! Byte-preserving RFC 1572 updates, with the C server's atomic storage limits.
use anyhow::{Result, ensure};
use std::collections::BTreeMap;
/// VAR and USERVAR occupy distinct namespaces; byte ordering is also display ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Var,
    UserVar,
}
/// Bounded environment owned by one connection, never applied to the process environment.
#[derive(Debug, Clone, Default)]
pub struct Environment(pub BTreeMap<(Kind, Vec<u8>), Vec<u8>>);
impl Environment {
    /// Parse and apply one IS replacement or INFO patch, committing only after complete validation.
    pub fn update(&mut self, p: &[u8]) -> Result<()> {
        ensure!(
            matches!(p.first(), Some(0 | 2)),
            "expected environment IS or INFO"
        );
        let mut next = if p[0] == 0 {
            Self::default()
        } else {
            self.clone()
        };
        let mut i = 1;
        let mut updates = 0;
        while i < p.len() {
            let kind = match p[i] {
                0 => Kind::Var,
                3 => Kind::UserVar,
                _ => anyhow::bail!("expected VAR or USERVAR"),
            };
            i += 1;
            updates += 1;
            ensure!(updates <= 64, "too many environment updates");
            let name = read(p, &mut i, 256)?;
            let value = if p.get(i) == Some(&1) {
                i += 1;
                Some(read(p, &mut i, 4096)?)
            } else {
                None
            };
            if let Some(v) = value {
                next.0.insert((kind, name), v);
            } else {
                next.0.remove(&(kind, name));
            }
            ensure!(
                next.0.len() <= 64
                    && next
                        .0
                        .iter()
                        .map(|((_, n), v)| n.len() + v.len())
                        .sum::<usize>()
                        <= 65536,
                "environment storage limit"
            );
        }
        *self = next;
        Ok(())
    }
}
/// Decode escaped bytes, stopping at the next unescaped structural marker.
fn read(p: &[u8], i: &mut usize, limit: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    while *i < p.len() {
        match p[*i] {
            0 | 1 | 3 => break,
            2 => {
                *i += 1;
                ensure!(*i < p.len(), "trailing environment escape");
            }
            _ => {}
        }
        out.push(p[*i]);
        *i += 1;
        ensure!(out.len() <= limit, "environment field limit");
    }
    Ok(out)
}
