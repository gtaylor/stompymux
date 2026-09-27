//! Shared restartable forced descent, with a BattleMech world adapter for shutdown falls.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// A forced-descent cursor. Horizontal position belongs to the enclosing unit.
/// Restarting a BattleMech engine does not arrest this descent: its jump jets are
/// not the sustained flight system used by aircraft recovery rules.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FreeFallRecord")]
pub struct BattleFreeFall {
    elevation: f64,
    /// Downward speed; zero retains the scheduled event while aircraft lift arrests descent.
    speed: u16,
    remaining: u8,
    #[serde(default)]
    grounded: bool,
}

/// Validate saved event timing before admitting a descent into the simulation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FreeFallRecord {
    elevation: f64,
    speed: u16,
    remaining: u8,
    #[serde(default)]
    grounded: bool,
}

impl TryFrom<FreeFallRecord> for BattleFreeFall {
    type Error = anyhow::Error;

    fn try_from(record: FreeFallRecord) -> Result<Self> {
        ensure!(
            (1..=3).contains(&record.remaining),
            "Invalid free-fall countdown"
        );
        ensure!(
            record.elevation.is_finite()
                && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&record.elevation),
            "Invalid free-fall altitude"
        );
        Ok(Self {
            elevation: record.elevation,
            speed: record.speed,
            remaining: record.remaining,
            grounded: record.grounded,
        })
    }
}

/// The enclosing transaction applies impact damage and removes the descent cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "Apply impact effects and remove the cursor in the same transaction"]
pub enum BattleFreeFallStep {
    Waiting,
    Descending,
    /// Sustained lift has arrested descent; the caller removes the cursor.
    Recovered,
    Impact {
        levels: u32,
    },
}

impl BattleFreeFall {
    /// Committed seconds until the next descent event.
    pub(super) fn remaining(self) -> u8 {
        self.remaining
    }

    /// Begin at the unit's integer altitude; the first event is due in three seconds.
    pub fn new(elevation: i32) -> Self {
        Self {
            elevation: f64::from(elevation),
            speed: 1,
            remaining: 3,
            grounded: false,
        }
    }

    /// Preserve continuous launch height until the first scheduled integer descent step.
    pub fn at_altitude(elevation: f64) -> Result<Self> {
        ensure!(
            elevation.is_finite()
                && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&elevation),
            "Invalid free-fall altitude"
        );
        Ok(Self {
            elevation,
            speed: 1,
            remaining: 3,
            grounded: false,
        })
    }

    /// Continuous altitude while waiting; subsequent descent events use whole terrain levels.
    pub fn altitude(self) -> f64 {
        self.elevation
    }

    /// Whether a landing ended airborne movement while leaving this event pending.
    pub fn grounded(self) -> bool {
        self.grounded
    }

    /// Landing retains event timing and speed until the scheduled impact is resolved.
    pub(super) fn land(&mut self) {
        self.grounded = true;
    }

    /// Scenario positioning changes height without restarting the descent clock or speed.
    pub(super) fn relocate(&mut self, elevation: f64) {
        self.elevation = elevation;
    }

    /// Current altitude, unchanged between scheduled descent events.
    pub fn elevation(self) -> i32 {
        self.elevation as i32
    }

    /// Advance one committed second against the currently applicable surface.
    /// Terrain selection and impact effects belong to the caller. Impact retains
    /// the pre-contact altitude for damage rules that inspect water or bridges.
    /// Gravity and map movement rates do not change this event's cadence.
    /// Invalid arithmetic and impact outcomes leave the cursor unchanged.
    pub fn advance(&mut self, surface: i32) -> Result<BattleFreeFallStep> {
        self.advance_with_lift(surface, false)
    }

    /// Aircraft lift brakes descent on the same clock used by unpowered units.
    pub(super) fn advance_with_lift(
        &mut self,
        surface: i32,
        lift: bool,
    ) -> Result<BattleFreeFallStep> {
        if self.remaining > 1 {
            self.remaining -= 1;
            return Ok(BattleFreeFallStep::Waiting);
        }
        if lift && self.speed == 0 {
            return Ok(BattleFreeFallStep::Recovered);
        }
        let speed = if lift {
            self.speed - 1
        } else {
            self.speed
                .checked_add(1)
                .context("Free-fall speed overflow")?
        };
        let height = i64::from(self.elevation()) - i64::from(surface);
        if height <= i64::from(speed) {
            let speed = u32::from(speed);
            return Ok(BattleFreeFallStep::Impact {
                levels: speed * (speed + 1) / 2,
            });
        }
        let elevation = self
            .elevation()
            .checked_sub(i32::from(speed))
            .context("Free-fall altitude overflow")?;
        *self = Self {
            elevation: f64::from(elevation),
            speed,
            remaining: 3,
            grounded: false,
        };
        Ok(BattleFreeFallStep::Descending)
    }
}

/// Advance one unit inside the enclosing airborne-motion transaction.
pub(super) fn advance_unit(
    world: &mut crate::World,
    id: crate::ObjectId,
    mut rules: super::BattleFallRules,
    character: bool,
    falls: &mut Vec<super::BattleFallReport>,
    feedback: (&mut Vec<super::BattlePilotNotice>, usize),
) -> Result<Vec<super::BattleNotice>> {
    let unit = &world.btech.constructed_units()[&id];
    let mut fall = unit.free_fall.context("Unit is not free-falling")?;
    let position = unit.position().context("Free-falling unit is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    if fall.grounded() {
        fall.elevation = f64::from(unit.elevation_level(tile));
    }
    let surface = super::fall_profile::surface(tile, fall.elevation());
    rules.toughness = unit
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    let step = fall.advance(i32::from(surface))?;
    if let BattleFreeFallStep::Impact { levels } = step {
        let levels =
            i16::try_from(levels).context("Free-fall impact exceeds supported severity")?;
        // Capture visibility before impact can destroy the falling unit or alter terrain.
        let observers = super::observer_messages(world, id, "hits the ground!");
        let report = if character && world.objects[&id].flags.contains(crate::Flag::InCharacter) {
            super::fall::resolve_character_signed_fall(world, id, levels, rules)?
        } else {
            super::fall::resolve_signed_fall(world, id, levels, rules)?
        };
        world.btech.constructed.get_mut(&id).unwrap().free_fall = None;
        let mut notices = vec![super::BattleNotice {
            unit: id,
            text: "You hit the ground!".to_owned(),
        }];
        notices.extend(
            observers
                .into_iter()
                .map(|(unit, text)| super::BattleNotice { unit, text }),
        );
        let mut private = Vec::new();
        report.append_notices(id, &mut notices, &mut private);
        super::piloting::append_feedback(feedback.0, private, feedback.1);
        falls.push(report);
        return Ok(notices);
    }
    world.btech.constructed.get_mut(&id).unwrap().free_fall = Some(fall);
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descent_waits_accelerates_and_replays_every_saved_second() -> Result<()> {
        let mut fall = BattleFreeFall::new(10);
        for (second, elevation) in [10, 10, 8, 8, 8, 5, 5, 5, 1, 1, 1, 1]
            .into_iter()
            .enumerate()
        {
            let mut restored: BattleFreeFall =
                serde_json::from_str(&serde_json::to_string(&fall)?)?;
            let result = fall.advance(0)?;
            assert_eq!(restored.advance(0)?, result);
            assert_eq!(restored, fall);
            assert_eq!(fall.elevation(), elevation);
            let expected = match second + 1 {
                3 | 6 | 9 => BattleFreeFallStep::Descending,
                12 => BattleFreeFallStep::Impact { levels: 15 },
                _ => BattleFreeFallStep::Waiting,
            };
            assert_eq!(result, expected);
        }
        Ok(())
    }

    #[test]
    fn contact_boundary_uses_live_surface_and_preserves_preimpact_height() -> Result<()> {
        for (elevation, surface) in [(2, 0), (0, 0), (-2, -4), (1, 5)] {
            let mut fall = BattleFreeFall::new(elevation);
            let _ = fall.advance(-9)?;
            let _ = fall.advance(-9)?;
            let before = fall;
            assert_eq!(
                fall.advance(surface)?,
                BattleFreeFallStep::Impact { levels: 3 }
            );
            assert_eq!(fall, before);
        }
        let mut below_deck = BattleFreeFall::new(1);
        let _ = below_deck.advance(-4)?;
        let _ = below_deck.advance(-4)?;
        assert_eq!(below_deck.advance(-4)?, BattleFreeFallStep::Descending);
        assert_eq!(below_deck.elevation(), -1);
        Ok(())
    }

    #[test]
    fn malformed_saved_events_are_rejected_and_overflow_is_atomic() -> Result<()> {
        for (speed, remaining) in [(1, 0), (1, 4)] {
            assert!(
                serde_json::from_value::<BattleFreeFall>(serde_json::json!({
                    "elevation": 5, "speed": speed, "remaining": remaining,
                }))
                .is_err()
            );
        }
        let mut fall: BattleFreeFall = serde_json::from_value(serde_json::json!({
            "elevation": 5, "speed": u16::MAX, "remaining": 1,
        }))?;
        let before = fall;
        assert!(fall.advance(0).is_err());
        assert_eq!(fall, before);
        Ok(())
    }
}
