//! Replayable unit dice streams carried by ordinary world checkpoints and unit persistence.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Explicit algorithm tag prevents a future generator change from reinterpreting saved state.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "algorithm", content = "state")]
enum Generator {
    #[serde(rename = "chacha8-v1")]
    ChaCha8(Box<ChaCha8Rng>),
}

/// Replayable generator plus an uncommitted, process-local journal of explicitly generic checks.
/// Journals clone with candidate worlds, but are neither persisted nor part of generator identity.
#[derive(Clone)]
pub struct BattleDice {
    generator: Generator,
    generic_rolls: super::BattleRollStatistics,
}

impl Serialize for BattleDice {
    /// Persist only the generator, directly supporting its full-width stream position.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.generator.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BattleDice {
    /// A restored generator begins a fresh process-local diagnostic journal.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            generator: Generator::deserialize(deserializer)?,
            generic_rolls: super::BattleRollStatistics::default(),
        })
    }
}

impl PartialEq for BattleDice {
    fn eq(&self, other: &Self) -> bool {
        self.generator == other.generator
    }
}

impl std::fmt::Debug for BattleDice {
    /// Diagnostics must not disclose the stream used for future game outcomes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BattleDice(chacha8-v1)")
    }
}

/// Generator position as plain values, stored in typed database columns.
///
/// The stream offset is split into a 64-bit block counter and a word within that
/// sixteen-word block, so every part fits a native SQLite integer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct BattleDiceState {
    /// ChaCha key.
    pub seed: [u8; 32],
    /// ChaCha nonce selecting one of the key's independent streams.
    pub stream: u64,
    /// Sixteen-word block containing the next output word.
    pub block: u64,
    /// Next output word within `block`, always below sixteen.
    pub word: u8,
}

impl BattleDice {
    /// Capture the generator position for typed storage.
    pub(crate) fn saved_state(&self) -> BattleDiceState {
        let Generator::ChaCha8(rng) = &self.generator;
        let position = rng.get_word_pos();
        BattleDiceState {
            seed: rng.get_seed(),
            stream: rng.get_stream(),
            block: (position / 16) as u64,
            word: (position % 16) as u8,
        }
    }

    /// Restore a generator from typed storage, beginning a fresh diagnostic journal.
    pub(crate) fn from_saved_state(state: BattleDiceState) -> Result<Self> {
        ensure!(state.word < 16, "Dice word offset is out of range");
        let mut rng = ChaCha8Rng::from_seed(state.seed);
        rng.set_stream(state.stream);
        rng.set_word_pos(u128::from(state.block) * 16 + u128::from(state.word));
        Ok(Self {
            generator: Generator::ChaCha8(Box::new(rng)),
            generic_rolls: super::BattleRollStatistics::default(),
        })
    }

    /// Reproducible stream for isolated scenarios and deterministic rule tests.
    pub fn seeded(seed: [u8; 32]) -> Self {
        Self {
            generator: Generator::ChaCha8(Box::new(ChaCha8Rng::from_seed(seed))),
            generic_rolls: super::BattleRollStatistics::default(),
        }
    }

    /// Independent stream for a newly constructed unit, saved before gameplay rolls occur.
    pub(crate) fn fresh() -> Self {
        Self::seeded(rand::random())
    }

    /// Roll a fair die with a nonzero number of faces, without modulo bias.
    pub fn die(&mut self, sides: u16) -> Result<u16> {
        ensure!(sides != 0, "A die must have at least one face");
        let sides = u32::from(sides);
        let Generator::ChaCha8(rng) = &mut self.generator;
        loop {
            let value = rng.next_u32();
            if value < u32::MAX - u32::MAX % sides {
                return Ok((value % sides + 1) as u16);
            }
        }
    }

    /// Draw a nonnegative 31-bit ranking value from one saved generator word.
    pub fn rank_i31(&mut self) -> u32 {
        let Generator::ChaCha8(rng) = &mut self.generator;
        rng.next_u32() >> 1
    }

    /// Roll one fair six-sided die.
    pub fn d6(&mut self) -> u8 {
        self.die(6).expect("six is a valid die size") as u8
    }

    /// Consciousness saving throw: ordinary 2d6, or the best two of 3d6 with toughness.
    pub fn consciousness_roll(&mut self, toughness: bool) -> u8 {
        let first = self.d6();
        let second = self.d6();
        if !toughness {
            return first + second;
        }
        let third = self.d6();
        first + second + third - first.min(second).min(third)
    }

    /// Record one reference generic check; direct dice and character checks use uncounted methods.
    pub fn generic_roll(&mut self) -> u8 {
        let roll = self.two_d6();
        self.generic_rolls
            .record(roll)
            .expect("generic roll journal capacity exceeded");
        roll
    }

    /// Inspect pending diagnostics separately from generator/gameplay equality.
    pub fn generic_roll_statistics(&self) -> &super::BattleRollStatistics {
        &self.generic_rolls
    }

    /// Transfer a committed stream journal to the simulation-owned history without drawing dice.
    pub fn take_generic_roll_statistics(&mut self) -> super::BattleRollStatistics {
        std::mem::take(&mut self.generic_rolls)
    }

    /// The sum of two independent dice used by ordinary BattleTech checks.
    pub fn two_d6(&mut self) -> u8 {
        self.d6() + self.d6()
    }
}

/// Consume a bounded group of Mech or vehicle dice within the caller's world transaction.
/// Game actions must validate their guards before calling this function and commit
/// their effects together with the updated world. It is not a player-facing reroll API.
pub fn roll_unit_dice(world: &mut World, id: ObjectId, count: u8) -> Result<Vec<u8>> {
    ensure!(
        (1..=20).contains(&count),
        "Dice count must be between 1 and 20"
    );
    let dice = unit_dice_mut(world, id)?;
    Ok((0..count).map(|_| dice.d6()).collect())
}

/// Borrow the owning unit's stream inside an already validated candidate transaction.
pub(super) fn unit_dice_mut(world: &mut World, id: ObjectId) -> Result<&mut BattleDice> {
    let unit = world
        .btech
        .unit_mut(id)
        .context("Unit construction state is unavailable")?;
    Ok(super::with_unit_mut!(unit, |unit| &mut unit.dice))
}

#[cfg(test)]
mod statistics_tests {
    use super::*;

    /// Classification changes neither random words nor persistence; discarded candidates keep independent journals.
    #[test]
    fn generic_journal_is_explicit_transactional_and_not_persisted() {
        let mut counted = BattleDice::seeded([37; 32]);
        let mut direct = counted.clone();
        for _ in 0..16 {
            assert_eq!(counted.generic_roll(), direct.two_d6());
        }
        assert_eq!(counted, direct);
        assert_eq!(counted.generic_roll_statistics().total(), 16);
        assert_eq!(direct.generic_roll_statistics().total(), 0);
        let mut discarded = counted.clone();
        discarded.generic_roll();
        assert_eq!(counted.generic_roll_statistics().total(), 16);
        assert_eq!(discarded.generic_roll_statistics().total(), 17);
        assert_eq!(
            serde_json::to_value(&counted).unwrap(),
            serde_json::to_value(&direct).unwrap()
        );
        let mut restored: BattleDice =
            serde_json::from_value(serde_json::to_value(&counted).unwrap()).unwrap();
        assert_eq!(restored.generic_roll_statistics().total(), 0);
        let history = counted.take_generic_roll_statistics();
        assert_eq!(history.total(), 16);
        assert_eq!(counted.generic_roll_statistics().total(), 0);
        assert_eq!(counted.two_d6(), restored.two_d6());
        assert_eq!(
            counted.consciousness_roll(true),
            restored.consciousness_roll(true)
        );
        assert_eq!(counted.generic_roll_statistics().total(), 0);
    }

    /// Typed storage restores the exact stream position, including mid-block offsets.
    #[test]
    fn saved_state_round_trips_stream_position() {
        let mut dice = BattleDice::seeded([11; 32]);
        for _ in 0..21 {
            dice.two_d6();
        }
        let state = dice.saved_state();
        assert!(state.word < 16);
        let mut restored = BattleDice::from_saved_state(state).unwrap();
        assert_eq!(restored, dice);
        for _ in 0..40 {
            assert_eq!(restored.two_d6(), dice.two_d6());
        }
        assert!(BattleDice::from_saved_state(BattleDiceState { word: 16, ..state }).is_err());
    }
}
