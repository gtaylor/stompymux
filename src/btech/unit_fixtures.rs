//! Typed setters that put BattleTech units, recoveries and maps into exact states.
//!
//! Scenario fixtures need states gameplay never writes directly, such as a seeded dice
//! stream or a hand-edited template. Each setter has the same effect as editing that field
//! through [`BtechState::rewrite_unit_record`], including clearing the runtime-only state a
//! serialization round trip drops, while checking field names and types at compile time.
//!
//! A Mech record decodes field by field, so Mech edits are written in place. A vehicle
//! record decodes by rebuilding the vehicle from its template and validating every saved
//! field against it, so vehicle edits still pass through the record to keep those checks.
use super::{
    BattleDice, BattleMotion, BattlePower, BattleTemplate, BattleUnit, BattleVehicleTemplate,
    BtechState,
};
use crate::ObjectId;
use anyhow::{Context, Result, bail};

/// A replacement construction template for either chassis.
#[derive(Debug, Clone)]
pub enum BattleUnitDefinition {
    /// A BattleMech template.
    Mech(BattleTemplate),
    /// A combat vehicle template.
    Vehicle(BattleVehicleTemplate),
}

impl From<BattleTemplate> for BattleUnitDefinition {
    fn from(definition: BattleTemplate) -> Self {
        Self::Mech(definition)
    }
}

impl From<BattleVehicleTemplate> for BattleUnitDefinition {
    fn from(definition: BattleVehicleTemplate) -> Self {
        Self::Vehicle(definition)
    }
}

/// Encode a typed value as the JSON a vehicle record holds for it.
fn encoded(value: impl serde::Serialize) -> Result<serde_json::Value> {
    Ok(serde_json::to_value(value)?)
}

impl BtechState {
    /// Apply one fixture edit: in place to a Mech, or through a vehicle's validated record.
    fn edit_unit_fixture(
        &mut self,
        id: ObjectId,
        mech: impl FnOnce(&mut BattleUnit) -> Result<()>,
        vehicle: impl FnOnce(&mut serde_json::Value) -> Result<()>,
    ) -> Result<()> {
        if let Some(unit) = self.constructed.get_mut(&id) {
            mech(unit)?;
            self.clear_runtime_state();
            return Ok(());
        }
        let mut outcome = Ok(());
        self.rewrite_unit_record(id, |record| outcome = vehicle(record))?;
        outcome
    }

    /// Replace a unit's dice stream, for example with [`BattleDice::seeded`].
    pub fn set_unit_dice(&mut self, id: ObjectId, dice: BattleDice) -> Result<()> {
        let record = encoded(&dice)?;
        self.edit_unit_fixture(
            id,
            |unit| {
                unit.dice = dice;
                Ok(())
            },
            |vehicle| {
                vehicle["dice"] = record;
                Ok(())
            },
        )
    }

    /// Replace the dice stream a unit's empty cockpit uses for crew recovery.
    pub fn set_unit_crew_recovery_dice(&mut self, id: ObjectId, dice: BattleDice) -> Result<()> {
        let record = encoded(&dice)?;
        self.edit_unit_fixture(
            id,
            |unit| {
                unit.crew_recovery.set_dice(dice);
                Ok(())
            },
            |vehicle| {
                vehicle["crew_recovery"]["dice"] = record;
                Ok(())
            },
        )
    }

    /// Set a unit's power state without running a startup or shutdown sequence.
    pub fn set_unit_power(&mut self, id: ObjectId, power: BattlePower) -> Result<()> {
        self.edit_unit_fixture(
            id,
            |unit| {
                unit.power = power;
                Ok(())
            },
            |vehicle| {
                vehicle["power"] = encoded(power)?;
                Ok(())
            },
        )
    }

    /// Replace a unit's construction template. The template must match the chassis.
    pub fn set_unit_definition(
        &mut self,
        id: ObjectId,
        definition: impl Into<BattleUnitDefinition>,
    ) -> Result<()> {
        match definition.into() {
            BattleUnitDefinition::Mech(definition) => {
                let unit = self
                    .constructed
                    .get_mut(&id)
                    .with_context(|| format!("#{} is not a Mech", id.0))?;
                unit.set_fixture_definition(definition);
                self.clear_runtime_state();
                Ok(())
            }
            BattleUnitDefinition::Vehicle(definition) => {
                if !self.vehicles.contains_key(&id) {
                    bail!("#{} is not a vehicle", id.0);
                }
                let record = encoded(&definition)?;
                self.rewrite_unit_record(id, |vehicle| vehicle["definition"] = record)
            }
        }
    }

    /// Replace the rounds remaining in every ammunition bin of a unit.
    pub fn set_unit_ammunition(&mut self, id: ObjectId, rounds: Vec<u16>) -> Result<()> {
        let record = encoded(&rounds)?;
        self.edit_unit_fixture(
            id,
            |unit| {
                unit.ammunition = rounds;
                Ok(())
            },
            |vehicle| {
                vehicle["ammunition"] = record;
                Ok(())
            },
        )
    }

    /// Set the rounds remaining in one of a unit's existing ammunition bins.
    pub fn set_unit_ammunition_bin(&mut self, id: ObjectId, bin: usize, rounds: u16) -> Result<()> {
        let missing = || format!("#{} has no ammunition bin {bin}", id.0);
        self.edit_unit_fixture(
            id,
            |unit| {
                *unit.ammunition.get_mut(bin).with_context(missing)? = rounds;
                Ok(())
            },
            |vehicle| {
                *vehicle["ammunition"].get_mut(bin).with_context(missing)? = rounds.into();
                Ok(())
            },
        )
    }

    /// Edit a placed unit's motion, such as its speed or heading.
    pub fn edit_unit_motion(
        &mut self,
        id: ObjectId,
        edit: impl FnOnce(&mut BattleMotion),
    ) -> Result<()> {
        let unplaced = || format!("#{} has no motion", id.0);
        if let Some(unit) = self.constructed.get_mut(&id) {
            edit(unit.motion.as_mut().with_context(unplaced)?);
            self.clear_runtime_state();
            return Ok(());
        }
        let mut outcome = Ok(());
        self.rewrite_unit_record(id, |vehicle| {
            outcome = (|| {
                let mut motion: Option<BattleMotion> =
                    serde_json::from_value(vehicle["motion"].take())?;
                edit(motion.as_mut().with_context(unplaced)?);
                vehicle["motion"] = encoded(motion)?;
                Ok(())
            })();
        })?;
        outcome
    }

    /// Replace a player's consciousness recovery dice stream.
    pub fn set_recovery_dice(&mut self, player: ObjectId, dice: BattleDice) -> Result<()> {
        self.recoveries
            .get_mut(&player)
            .with_context(|| format!("#{} has no recovery record", player.0))?
            .set_dice(dice);
        self.clear_runtime_state();
        Ok(())
    }

    /// Replace a map's fire spread dice stream if it has one, reporting whether it did.
    /// A map without a stream keeps none, because whether one exists changes ignition.
    pub fn replace_map_fire_dice(&mut self, map: ObjectId, dice: BattleDice) -> Result<bool> {
        let stream = self
            .maps
            .get_mut(&map)
            .with_context(|| format!("#{} has no map record", map.0))?
            .fire_dice
            .as_mut();
        let Some(stream) = stream else {
            return Ok(false);
        };
        *stream = dice;
        self.clear_runtime_state();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BattleMapAsset, Config, Kind, World};

    /// A placed Mech and vehicle on a small map, with one player recovery.
    fn world() -> (World, ObjectId, ObjectId, ObjectId) {
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World {
            next_id: 42,
            ..Default::default()
        };
        let mech = world.create(&config, "Mech".into(), Kind::Thing);
        let vehicle = world.create(&config, "Vehicle".into(), Kind::Thing);
        let map = world.create(&config, "Map".into(), Kind::Room);
        super::super::create_unit(
            &mut world,
            mech,
            BattleTemplate::parse("JR7-D", include_str!("../../game/mechs/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        crate::create_battle_vehicle(
            &mut world,
            vehicle,
            BattleVehicleTemplate::parse(
                "Demolisher",
                include_str!("../../game/mechs/Demolisher.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        super::super::create_map(
            &mut world,
            map,
            "test",
            BattleMapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
        )
        .unwrap();
        super::super::place_unit(&mut world, mech, map, 0, 0).unwrap();
        super::super::place_unit(&mut world, vehicle, map, 1, 1).unwrap();
        (world, mech, vehicle, map)
    }

    /// Each setter leaves the same state as editing that field through the record.
    #[test]
    fn setters_match_editing_the_serialized_record() {
        let (world, mech, vehicle, _) = world();
        for id in [mech, vehicle] {
            let check = |set: &dyn Fn(&mut BtechState), edit: &dyn Fn(&mut serde_json::Value)| {
                let mut typed = world.btech.clone();
                typed.retire_sanctions.borrow_mut().insert(id);
                let mut json = typed.clone();
                set(&mut typed);
                json.rewrite_unit_record(id, edit).unwrap();
                assert_eq!(typed, json);
            };
            let dice = BattleDice::seeded([7; 32]);
            check(
                &|state| state.set_unit_dice(id, dice.clone()).unwrap(),
                &|record| record["dice"] = serde_json::to_value(&dice).unwrap(),
            );
            check(
                &|state| state.set_unit_crew_recovery_dice(id, dice.clone()).unwrap(),
                &|record| record["crew_recovery"]["dice"] = serde_json::to_value(&dice).unwrap(),
            );
            check(
                &|state| state.set_unit_power(id, BattlePower::Running).unwrap(),
                &|record| {
                    record["power"] = serde_json::to_value(BattlePower::Running).unwrap();
                },
            );
            check(
                &|state| state.set_unit_ammunition_bin(id, 0, 3).unwrap(),
                &|record| record["ammunition"][0] = 3.into(),
            );
            let class = if id == mech {
                "constructed"
            } else {
                "vehicles"
            };
            let bins =
                serde_json::to_value(&world.btech).unwrap()[class][id.0.to_string()]["ammunition"]
                    .as_array()
                    .unwrap()
                    .len();
            let rounds: Vec<u16> = (1..=bins as u16).collect();
            check(
                &|state| state.set_unit_ammunition(id, rounds.clone()).unwrap(),
                &|record| record["ammunition"] = serde_json::to_value(&rounds).unwrap(),
            );
            check(
                &|state| {
                    state
                        .edit_unit_motion(id, |motion| {
                            motion.heading = 60.0;
                            motion.desired_heading = 60.0;
                        })
                        .unwrap();
                },
                &|record| {
                    record["motion"]["heading"] = 60.0.into();
                    record["motion"]["desired_heading"] = 60.0.into();
                },
            );
        }
        let mut definition = world.btech.constructed_units()[&mech].definition().clone();
        definition.max_speed = 64.5;
        let mut typed = world.btech.clone();
        let mut json = world.btech.clone();
        typed.set_unit_definition(mech, definition.clone()).unwrap();
        json.rewrite_unit_record(mech, |record| {
            record["definition"] = serde_json::to_value(&definition).unwrap();
        })
        .unwrap();
        assert_eq!(typed, json);
    }

    /// Setters reject missing records, missing bins and a template of the wrong chassis.
    #[test]
    fn setters_reject_what_the_record_cannot_hold() {
        let (mut world, mech, vehicle, map) = world();
        let template = world.btech.constructed_units()[&mech].definition().clone();
        assert!(world.btech.set_unit_definition(vehicle, template).is_err());
        assert!(world.btech.set_unit_power(map, BattlePower::Off).is_err());
        assert!(world.btech.set_unit_ammunition_bin(mech, 99, 1).is_err());
        // A vehicle's record still checks its bins against its template.
        assert!(
            world
                .btech
                .set_unit_ammunition(vehicle, vec![1; 99])
                .is_err()
        );
        assert!(
            world
                .btech
                .set_recovery_dice(mech, BattleDice::fresh())
                .is_err()
        );
    }
}
