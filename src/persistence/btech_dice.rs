//! Typed columns for saved dice streams, shared by every table that owns one.
//!
//! A stream is stored as four columns: `dice_seed` (32-byte key), `dice_stream`,
//! `dice_block` and `dice_word`. Unsigned 64-bit values are stored bit for bit in
//! SQLite's signed 64-bit integers.
use super::write::Cell;
use crate::{Dice, btech::DiceState};
use anyhow::{Context, Result};
use sqlx::{Row, sqlite::SqliteRow};

/// Column list for SELECT statements that read a stream.
pub(super) const COLUMNS: &str = "dice_seed,dice_stream,dice_block,dice_word";

/// Owned column values for a stream, ready for a row write.
pub(super) fn fields(dice: &Dice) -> [(&'static str, Cell); 4] {
    let state = dice.saved_state();
    [
        ("dice_seed", Cell::Blob(state.seed.to_vec())),
        ("dice_stream", Cell::Integer(state.stream as i64)),
        ("dice_block", Cell::Integer(state.block as i64)),
        ("dice_word", Cell::Integer(i64::from(state.word))),
    ]
}

/// Rebuild a stream from a row selected with [`COLUMNS`].
pub(super) fn read(row: &SqliteRow) -> Result<Dice> {
    let seed: Vec<u8> = row.try_get("dice_seed")?;
    let word: i64 = row.try_get("dice_word")?;
    Dice::from_saved_state(DiceState {
        seed: seed.try_into().ok().context("Dice seed must be 32 bytes")?,
        stream: row.try_get::<i64, _>("dice_stream")? as u64,
        block: row.try_get::<i64, _>("dice_block")? as u64,
        word: u8::try_from(word).context("Dice word offset is out of range")?,
    })
}
