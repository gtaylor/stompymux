//! Chassis-neutral reads and validated edits of BattleTech Mechs and vehicles.
//!
//! [`BattleUnitRef`] reads the state both chassis share without branching on which store
//! holds a unit. [`BtechState::edit_unit`] applies a batch of edits to a draft copy of
//! either chassis, validates the result once, and commits it only when it is valid, so a
//! sequence of changes may pass through states that would be invalid on their own. The
//! single-field setters wrap one-edit batches. Scenario fixtures use these to put units
//! into exact states, such as a seeded dice stream or a hand-edited template; build a
//! deliberately invalid unit through [`BtechState::rewrite_unit_record`] instead.
use super::{
    BattleCharacterPilotStatus, BattleContact, BattleDice, BattleFireMode, BattleFreeFall,
    BattleMotion, BattlePosition, BattlePower, BattleTargetLock, BattleTemplate, BattleUnit,
    BattleUnitSignature, BattleVehicle, BattleVehicleTemplate, BattleWeaponReadiness, BtechState,
};
use crate::ObjectId;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;

/// Read access to a Mech or vehicle through the state both chassis share.
#[derive(Debug, Clone, Copy)]
pub enum BattleUnitRef<'a> {
    /// A constructed BattleMech.
    Mech(&'a BattleUnit),
    /// A combat vehicle.
    Vehicle(&'a BattleVehicle),
}

/// Forward shared getters to whichever chassis the reference holds.
macro_rules! shared_getters {
    ($($(#[$doc:meta])* fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $ret:ty;)*) => {
        $(
            $(#[$doc])*
            pub fn $name(&self $(, $arg: $ty)*) -> $ret {
                match self {
                    Self::Mech(unit) => unit.$name($($arg),*),
                    Self::Vehicle(vehicle) => vehicle.$name($($arg),*),
                }
            }
        )*
    };
}

impl<'a> BattleUnitRef<'a> {
    /// Whether the unit is a BattleMech.
    pub fn is_mech(&self) -> bool {
        matches!(self, Self::Mech(_))
    }

    /// Rounds remaining in each ammunition bin.
    pub fn ammunition(&self) -> &'a [u16] {
        match *self {
            Self::Mech(unit) => unit.ammunition(),
            Self::Vehicle(vehicle) => vehicle.ammunition(),
        }
    }

    /// Weapons still recycling, by weapon index, with seconds remaining.
    pub fn weapon_recycle(&self) -> &'a BTreeMap<usize, u16> {
        match *self {
            Self::Mech(unit) => unit.weapon_recycle(),
            Self::Vehicle(vehicle) => vehicle.weapon_recycle(),
        }
    }

    /// Contacts the unit's sensors currently hold.
    pub fn contacts(&self) -> &'a BTreeMap<ObjectId, BattleContact> {
        match *self {
            Self::Mech(unit) => unit.contacts(),
            Self::Vehicle(vehicle) => vehicle.contacts(),
        }
    }

    shared_getters! {
        /// Map placement, if the unit is on a battlefield.
        fn position(&self) -> Option<BattlePosition>;
        /// Continuous movement state of a placed unit.
        fn motion(&self) -> Option<BattleMotion>;
        /// Reactor or engine state.
        fn power(&self) -> BattlePower;
        /// Whether the unit has been destroyed.
        fn is_destroyed(&self) -> bool;
        /// The player piloting the unit.
        fn pilot(&self) -> Option<ObjectId>;
        /// Injuries the pilot has taken.
        fn pilot_injuries(&self) -> u8;
        /// Character-system pilot condition, when one is tracked.
        fn character_pilot_status(&self) -> Option<BattleCharacterPilotStatus>;
        /// An active fall, if the unit is falling.
        fn free_fall(&self) -> Option<BattleFreeFall>;
        /// Seconds of inferno burning left.
        fn inferno_remaining(&self) -> u32;
        /// The unit's battlefield slot.
        fn map_slot(&self) -> Option<u32>;
        /// Team and optical signature.
        fn signature(&self) -> BattleUnitSignature;
        /// Tactical label shown to other units.
        fn battlefield_id(&self) -> Option<String>;
        /// The target the unit has locked, if any.
        fn target_lock(&self) -> Option<BattleTargetLock>;
        /// Whether one weapon can fire now, and why not if it cannot.
        fn weapon_readiness(&self, index: usize) -> Result<BattleWeaponReadiness>;
        /// One weapon's selected fire mode.
        fn fire_mode(&self, index: usize) -> Result<BattleFireMode>;
    }
}

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

/// The draft copy an edit changes.
enum Draft {
    Mech(Box<BattleUnit>),
    Vehicle(Box<BattleVehicle>),
}

/// A batch of edits to one unit's draft, validated together when the batch ends.
///
/// Edits that cannot apply, such as a bin the unit lacks, are reported by
/// [`BtechState::edit_unit`] and abandon the whole batch.
pub struct BattleUnitEdit {
    draft: Draft,
    error: Option<anyhow::Error>,
}

impl BattleUnitEdit {
    /// Record the first edit that could not apply.
    fn fail(&mut self, error: anyhow::Error) {
        self.error.get_or_insert(error);
    }

    /// Replace the unit's dice stream, for example with [`BattleDice::seeded`].
    pub fn set_dice(&mut self, dice: BattleDice) {
        match &mut self.draft {
            Draft::Mech(unit) => unit.dice = dice,
            Draft::Vehicle(vehicle) => vehicle.dice = dice,
        }
    }

    /// Replace the dice stream the unit's empty cockpit uses for crew recovery.
    pub fn set_crew_recovery_dice(&mut self, dice: BattleDice) {
        match &mut self.draft {
            Draft::Mech(unit) => unit.crew_recovery.set_dice(dice),
            Draft::Vehicle(vehicle) => vehicle.crew_recovery.set_dice(dice),
        }
    }

    /// Set the power state without running a startup or shutdown sequence.
    pub fn set_power(&mut self, power: BattlePower) {
        match &mut self.draft {
            Draft::Mech(unit) => unit.power = power,
            Draft::Vehicle(vehicle) => vehicle.power = power,
        }
    }

    /// Replace the construction template. It must match the unit's chassis.
    pub fn set_definition(&mut self, definition: impl Into<BattleUnitDefinition>) {
        match (&mut self.draft, definition.into()) {
            (Draft::Mech(unit), BattleUnitDefinition::Mech(definition)) => {
                unit.set_fixture_definition(definition)
            }
            (Draft::Vehicle(vehicle), BattleUnitDefinition::Vehicle(definition)) => {
                vehicle.set_fixture_definition(definition)
            }
            _ => self.fail(anyhow!("the template is for a different chassis")),
        }
    }

    /// Replace the rounds remaining in every ammunition bin.
    pub fn set_ammunition(&mut self, rounds: Vec<u16>) {
        match &mut self.draft {
            Draft::Mech(unit) => unit.ammunition = rounds,
            Draft::Vehicle(vehicle) => vehicle.ammunition = rounds,
        }
    }

    /// Set the rounds remaining in one existing ammunition bin.
    pub fn set_ammunition_bin(&mut self, bin: usize, rounds: u16) {
        let slot = match &mut self.draft {
            Draft::Mech(unit) => unit.ammunition.get_mut(bin),
            Draft::Vehicle(vehicle) => vehicle.ammunition.get_mut(bin),
        };
        match slot {
            Some(slot) => *slot = rounds,
            None => self.fail(anyhow!("the unit has no ammunition bin {bin}")),
        }
    }

    /// Edit a placed unit's motion, such as its speed or heading.
    pub fn edit_motion(&mut self, edit: impl FnOnce(&mut BattleMotion)) {
        let motion = match &mut self.draft {
            Draft::Mech(unit) => unit.motion.as_mut(),
            Draft::Vehicle(vehicle) => vehicle.motion.as_mut(),
        };
        match motion {
            Some(motion) => edit(motion),
            None => self.fail(anyhow!("the unit has no motion")),
        }
    }
}

impl BtechState {
    /// Read a Mech or vehicle through the state both chassis share.
    pub fn unit(&self, id: ObjectId) -> Option<BattleUnitRef<'_>> {
        if let Some(unit) = self.constructed.get(&id) {
            return Some(BattleUnitRef::Mech(unit));
        }
        self.vehicles.get(&id).map(BattleUnitRef::Vehicle)
    }

    /// Apply a batch of edits to one Mech or vehicle and validate the result once.
    ///
    /// The edits change a draft copy, so the batch may pass through states that would be
    /// invalid on their own, such as cutting power before clearing the throttle. The draft
    /// replaces the unit only if every edit applied and the result passes the same
    /// validation a saved record must pass when the server loads it; otherwise the
    /// state is unchanged. Like a record rewrite, a committed edit clears the runtime-only
    /// state a serialization round trip drops.
    pub fn edit_unit(
        &mut self,
        id: ObjectId,
        edit: impl FnOnce(&mut BattleUnitEdit),
    ) -> Result<()> {
        let draft = if let Some(unit) = self.constructed.get(&id) {
            Draft::Mech(Box::new(unit.clone()))
        } else if let Some(vehicle) = self.vehicles.get(&id) {
            Draft::Vehicle(Box::new(vehicle.clone()))
        } else {
            bail!("#{} has no unit or vehicle record", id.0);
        };
        let mut batch = BattleUnitEdit { draft, error: None };
        edit(&mut batch);
        if let Some(error) = batch.error {
            return Err(error.context(format!("editing #{}", id.0)));
        }
        match batch.draft {
            Draft::Mech(unit) => {
                unit.validate()
                    .with_context(|| format!("editing #{}", id.0))?;
                self.constructed.insert(id, *unit);
            }
            Draft::Vehicle(vehicle) => {
                let vehicle = vehicle
                    .restored()
                    .with_context(|| format!("editing #{}", id.0))?;
                self.vehicles.insert(id, vehicle);
            }
        }
        self.clear_runtime_state();
        Ok(())
    }

    /// Replace a unit's dice stream. See [`BattleUnitEdit::set_dice`].
    pub fn set_unit_dice(&mut self, id: ObjectId, dice: BattleDice) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_dice(dice))
    }

    /// Replace a unit's crew recovery dice. See [`BattleUnitEdit::set_crew_recovery_dice`].
    pub fn set_unit_crew_recovery_dice(&mut self, id: ObjectId, dice: BattleDice) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_crew_recovery_dice(dice))
    }

    /// Set a unit's power state. See [`BattleUnitEdit::set_power`].
    pub fn set_unit_power(&mut self, id: ObjectId, power: BattlePower) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_power(power))
    }

    /// Replace a unit's construction template. See [`BattleUnitEdit::set_definition`].
    pub fn set_unit_definition(
        &mut self,
        id: ObjectId,
        definition: impl Into<BattleUnitDefinition>,
    ) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_definition(definition))
    }

    /// Replace a unit's ammunition. See [`BattleUnitEdit::set_ammunition`].
    pub fn set_unit_ammunition(&mut self, id: ObjectId, rounds: Vec<u16>) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_ammunition(rounds))
    }

    /// Set one bin's rounds. See [`BattleUnitEdit::set_ammunition_bin`].
    pub fn set_unit_ammunition_bin(&mut self, id: ObjectId, bin: usize, rounds: u16) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_ammunition_bin(bin, rounds))
    }

    /// Edit a placed unit's motion. See [`BattleUnitEdit::edit_motion`].
    pub fn edit_unit_motion(
        &mut self,
        id: ObjectId,
        edit: impl FnOnce(&mut BattleMotion),
    ) -> Result<()> {
        self.edit_unit(id, |unit| unit.edit_motion(edit))
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

    /// Invalid edits fail for either chassis and leave the state unchanged.
    #[test]
    fn invalid_edits_fail_for_both_chassis_without_changing_state() {
        let (mut world, mech, vehicle, map) = world();
        let before = world.btech.clone();
        for id in [mech, vehicle] {
            let capacity = 9_999;
            assert!(
                world
                    .btech
                    .set_unit_ammunition_bin(id, 0, capacity)
                    .is_err()
            );
            assert!(world.btech.set_unit_ammunition_bin(id, 99, 1).is_err());
            assert!(world.btech.set_unit_ammunition(id, vec![1; 99]).is_err());
        }
        let template = world.btech.constructed_units()[&mech].definition().clone();
        assert!(world.btech.set_unit_definition(vehicle, template).is_err());
        assert!(world.btech.set_unit_power(map, BattlePower::Off).is_err());
        assert!(
            world
                .btech
                .set_recovery_dice(mech, BattleDice::fresh())
                .is_err()
        );
        assert_eq!(world.btech, before);
    }

    /// A batch validates once, so it may pass through states invalid on their own.
    #[test]
    fn a_batch_validates_only_its_result() {
        let (mut world, _, vehicle, _) = world();
        world
            .btech
            .edit_unit(vehicle, |unit| {
                unit.set_power(BattlePower::Running);
                unit.edit_motion(|motion| motion.desired_speed = 10.0);
            })
            .unwrap();
        // Cutting power first leaves a throttle on an inactive vehicle, which is invalid.
        let mut stepwise = world.btech.clone();
        assert!(stepwise.set_unit_power(vehicle, BattlePower::Off).is_err());
        world
            .btech
            .edit_unit(vehicle, |unit| {
                unit.set_power(BattlePower::Off);
                unit.edit_motion(|motion| motion.desired_speed = 0.0);
            })
            .unwrap();
        let unit = world.btech.unit(vehicle).unwrap();
        assert_eq!(unit.power(), BattlePower::Off);
        assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
    }

    /// The shared read view reports what each chassis's own getters report.
    #[test]
    fn unit_reference_reads_either_chassis() {
        let (world, mech, vehicle, map) = world();
        let unit = world.btech.unit(mech).unwrap();
        let own = &world.btech.constructed_units()[&mech];
        assert!(unit.is_mech());
        assert_eq!(unit.position(), own.position());
        assert_eq!(unit.ammunition(), own.ammunition());
        let unit = world.btech.unit(vehicle).unwrap();
        let own = &world.btech.vehicles()[&vehicle];
        assert!(!unit.is_mech());
        assert_eq!(unit.motion(), own.motion());
        assert_eq!(unit.weapon_readiness(0).ok(), own.weapon_readiness(0).ok());
        assert!(world.btech.unit(map).is_none());
    }
}
