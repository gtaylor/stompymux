//! Chassis-neutral reads and validated edits of BattleTech Mechs and vehicles.
//!
//! [`UnitRef`] reads the state both chassis share without branching on which store
//! holds a unit. [`BtechState::edit_unit`] applies a batch of edits to a draft copy of
//! either chassis, validates the result once, and commits it only when it is valid, so a
//! sequence of changes may pass through states that would be invalid on their own. The
//! single-field setters wrap one-edit batches. Scenario fixtures use these to put units
//! into exact states, such as a seeded dice stream or a hand-edited template; build a
//! deliberately invalid unit through [`BtechState::rewrite_unit_record`] instead.
use super::*;
use crate::ObjectId;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;

/// Run one body against whichever chassis a [`UnitRef`] holds.
///
/// The body is compiled once per chassis, so it may use any field or method both define
/// under the same name, including those whose section or loadout types differ.
macro_rules! with_unit {
    ($unit:expr, |$name:ident| $body:expr) => {
        match $unit {
            $crate::btech::UnitRef::Mech($name) => $body,
            $crate::btech::UnitRef::Vehicle($name) => $body,
        }
    };
}
pub(crate) use with_unit;

/// Run one body against whichever chassis a [`UnitMut`] holds; see [`with_unit`].
macro_rules! with_unit_mut {
    ($unit:expr, |$name:ident| $body:expr) => {
        match $unit {
            $crate::btech::UnitMut::Mech($name) => $body,
            $crate::btech::UnitMut::Vehicle($name) => $body,
        }
    };
}
pub(crate) use with_unit_mut;

/// Read access to a Mech or vehicle through the state both chassis share.
#[derive(Debug, Clone, Copy)]
pub enum UnitRef<'a> {
    /// A constructed BattleMech.
    Mech(&'a Mech),
    /// A combat vehicle.
    Vehicle(&'a Vehicle),
}

impl<'a> From<&'a Mech> for UnitRef<'a> {
    fn from(unit: &'a Mech) -> Self {
        Self::Mech(unit)
    }
}

impl<'a> From<&'a Vehicle> for UnitRef<'a> {
    fn from(vehicle: &'a Vehicle) -> Self {
        Self::Vehicle(vehicle)
    }
}

/// Forward methods both chassis define identically to whichever one a reference holds.
macro_rules! forward {
    (ref: $($(#[$doc:meta])* $vis:vis fn $name:ident(&self $(, $arg:ident: $ty:ty)*) $(-> $ret:ty)?;)*) => {
        $(
            $(#[$doc])*
            $vis fn $name(&self $(, $arg: $ty)*) $(-> $ret)? {
                match *self {
                    Self::Mech(unit) => unit.$name($($arg),*),
                    Self::Vehicle(vehicle) => vehicle.$name($($arg),*),
                }
            }
        )*
    };
    (mut: $($(#[$doc:meta])* $vis:vis fn $name:ident(&mut self $(, $arg:ident: $ty:ty)*) $(-> $ret:ty)?;)*) => {
        $(
            $(#[$doc])*
            $vis fn $name(&mut self $(, $arg: $ty)*) $(-> $ret)? {
                match self {
                    Self::Mech(unit) => unit.$name($($arg),*),
                    Self::Vehicle(vehicle) => vehicle.$name($($arg),*),
                }
            }
        )*
    };
}

/// Read fields both chassis store under the same name and type.
macro_rules! shared_fields {
    ($($(#[$doc:meta])* $name:ident: $ty:ty;)*) => {
        $(
            $(#[$doc])*
            pub(super) fn $name(&self) -> $ty {
                match *self {
                    Self::Mech(unit) => unit.$name,
                    Self::Vehicle(vehicle) => vehicle.$name,
                }
            }
        )*
    };
}

impl<'a> UnitRef<'a> {
    /// The pilot skill this chassis tests, or `None` for a chassis nobody steers.
    pub(super) fn piloting_skill(&self, extended: bool) -> Option<&'static str> {
        match *self {
            Self::Mech(unit) => Some(unit.chassis().piloting_skill(extended)),
            Self::Vehicle(vehicle) => vehicle.definition().movement.piloting_skill(extended),
        }
    }

    /// Top speed the chassis can reach in its current material condition, before load and terrain.
    pub(super) fn maximum_speed(&self) -> f64 {
        match *self {
            Self::Mech(unit) => unit.mobility().maximum_speed,
            Self::Vehicle(vehicle) => vehicle.maximum_speed(),
        }
    }

    /// Whether the unit is a BattleMech.
    pub fn is_mech(&self) -> bool {
        matches!(self, Self::Mech(_))
    }

    forward! { ref:
        /// The bin a self-destruct detonates: the most destructive one, first on a tie.
        pub fn largest_ammunition_hazard_bin(&self) -> Result<Option<usize>>;
        /// Whether TAG is installed, and whether it still works.
        pub(super) fn tag_hardware(&self) -> Result<(bool, bool)>;
        /// Remaining complete salvos, ordered by the resolved ammunition bins.
        pub fn ammunition(&self) -> &'a [u16];
        /// Plan up to `rounds` compatible rounds without changing inventory, mode, heat or dice.
        /// Prefer the selected section, then the mount and canonical section/slot order; empty or unavailable bins are skipped.
        /// A short plan exposes shortage so a firing mode can choose its specified fallback atomically.
        pub fn ammunition_feed(&self, index: usize, rounds: u16) -> Result<Vec<AmmunitionDraw>>;
        /// Current selected ammunition type; independent of bin inventory and recycle readiness.
        pub fn ammunition_mode(&self, index: usize) -> Result<AmmunitionMode>;
        /// Whether low ammunition notifies the occupants.
        pub fn ammunition_warning(&self) -> bool;
        /// Pilot-selected automatic defense state, initially disabled.
        pub fn ams_enabled(&self) -> bool;
        /// Whether armor threshold changes notify the occupants.
        pub fn armor_warning(&self) -> bool;
        /// Inspect a linked, live controller without altering the selected ammunition mode.
        pub fn artemis_operational(&self, index: usize) -> Result<bool>;
        /// Accumulated trajectory correction for the currently selected target.
        pub fn artillery_adjustment(&self) -> u8;
        /// Jumping and stabilization precede ground speed and the template's walking threshold.
        pub fn attacker_movement_modifier(&self, fasa_turning: bool) -> u8;
        /// Whether the pilot has opted to fall off cliffs without an avoidance check.
        pub fn auto_fall(&self) -> bool;
        /// Whether routine contact notices include targets whose reactors are not running.
        pub fn autocon_shutdown(&self) -> bool;
        /// Scenario identity or a stable base-36 label derived from the saved membership slot.
        pub fn battlefield_id(&self) -> Option<String>;
        /// The unit's durable display choices.
        pub fn brief_settings(&self) -> BriefSettings;
        /// Current BTHDebug configuration; attack reports do not consume this flag.
        pub fn bth_debug(&self) -> bool;
        /// Derive computer capabilities without trusting template flags or caching damage.
        pub fn c3_hardware(&self) -> Result<C3Hardware>;
        /// Classic C3 requires a live computer; losing every installed master disables its unit's C3.
        pub fn c3_operational(&self) -> Result<bool>;
        /// Saved character-mode injury count and fatal status, separate from tactical injury rules.
        pub fn character_pilot_status(&self) -> Option<super::CharacterPilotStatus>;
        /// Last observed contacts, which must be refreshed before being used as current visibility.
        pub fn contacts(&self) -> &'a BTreeMap<ObjectId, Contact>;
        /// Saved virtual-crew recovery; a present pilot owns their personal recovery instead.
        pub fn crew_recovery(&self) -> &'a super::Recovery;
        /// Current gameplay mass in 1/1024 tons, including an administrative correction.
        pub fn effective_mass(&self) -> Result<u32>;
        /// Guardian presence is sufficient; a biped Angel suite needs two surviving slots.
        /// A destroyed or flooded part disables the whole corresponding suite family.
        pub fn electronic_suite_available(&self, suite: ElectronicSuite) -> Result<bool>;
        /// Saved electronic controls and last committed field observation.
        pub fn electronics(&self) -> Electronics;
        /// Current persisted shooting-XP policy for this unit.
        pub fn experience_settings(&self) -> UnitExperience;
        /// Current firing mode, distinct from the template's initial equipment flags.
        pub fn fire_mode(&self, index: usize) -> Result<FireMode>;
        /// A completed launch marks this unit until the next committed heartbeat, including launches that miss.
        pub fn fired_recently(&self) -> bool;
        /// Pending unpowered vertical descent, including its durable event countdown.
        pub fn free_fall(&self) -> Option<super::FreeFall>;
        /// Whether the pilot has enabled friendly-fire protection.
        pub fn friendly_fire_safety(&self) -> bool;
        /// Any surviving section carrying this pod effect.
        pub fn has_beacon(&self, kind: BeaconKind) -> bool;
        /// Current coordinate target, independent of visibility or an occupying unit.
        pub fn hex_lock(&self) -> Option<HexLock>;
        /// Elapsed committed hide checks; zero is the initial scheduled event.
        pub fn hide_elapsed(&self) -> Option<u16>;
        /// Remaining inferno seconds; positive duration suppresses six points of heat dissipation.
        pub fn inferno_remaining(&self) -> u32;
        /// Core structure, cockpit loss or three engine hits destroy the unit, not its MUX object.
        pub fn is_destroyed(&self) -> bool;
        /// Administrator-controlled observer mode; cockpit pilots cannot enable it through radio settings.
        pub fn is_observer(&self) -> bool;
        /// Unix time supplied at the most recent completed startup, initially zero.
        pub fn last_startup(&self) -> i64;
        /// The unit's position in its current battlefield membership list.
        pub fn map_slot(&self) -> Option<u32>;
        /// Continuous motion, defaulting to the placed hex center before its first update.
        pub fn motion(&self) -> Option<super::Motion>;
        /// Whether the MechWarrior weapon-safety preference is enabled.
        pub fn mw_safety(&self) -> bool;
        /// Current cocoon or jump-jet descent, independent of ordinary jump flight.
        pub fn orbital_drop(&self) -> Option<OrbitalDrop>;
        /// Player currently occupying the cockpit, independent of passengers inside the unit.
        pub fn pilot(&self) -> Option<ObjectId>;
        /// Current cockpit injury count; confirmed pilot death is an independent event.
        pub fn pilot_injuries(&self) -> u8;
        /// Current battlefield coordinates, absent when the unit is off-map.
        pub fn position(&self) -> Option<Position>;
        /// Current engine state and pending startup countdown.
        pub fn power(&self) -> super::Power;
        /// Quality zero uses the chassis default; nonzero radio range overrides its derived reach.
        pub fn radio_capabilities(&self) -> RadioCapabilities;
        /// Active channel settings in letter order, starting with channel A.
        pub fn radio_channels(&self) -> &'a [RadioChannel];
        /// Simulation seconds until another interfered reception can attempt communication XP.
        pub fn radio_experience_remaining(&self) -> u8;
        /// Saved communication target used by reception interference until the next startup.
        pub fn radio_skill(&self) -> i16;
        /// Perception target captured at startup completion, used throughout that engine run.
        pub fn scanner_perception(&self) -> i16;
        /// Persisted lamp and switch state, independent of the unit's current illumination.
        pub fn searchlight(&self) -> Searchlight;
        /// Whether changes in external illumination notify the occupants.
        pub fn searchlight_warning(&self) -> bool;
        /// Current admitted self-destruct sequence, independent of pilot reassignment.
        pub fn self_destruct(&self) -> Option<SelfDestruct>;
        /// Scenario protection from ammunition self-destruct admission.
        pub fn self_destruct_safe(&self) -> bool;
        /// Nonzero template ranges override technology-base defaults independently.
        /// Template zero selects defaults; runtime zero remains zero. Subsequent sensor hits degrade ranges.
        pub fn sensor_ranges(&self) -> SensorRanges;
        /// Saved team and target visibility facts.
        pub fn signature(&self) -> UnitSignature;
        /// Self-selection declares this unit a spotter; another ID selects a forward observer.
        pub fn spotter(&self) -> Option<ObjectId>;
        /// Read pending radio connections and periodic checks without advancing their clocks.
        pub fn spotter_events(&self) -> &'a super::SpotterEvents;
        /// Saved TAG selection and countdown; current geometry is checked separately.
        pub fn tag(&self) -> TagState;
        /// Standalone TAG and integrated C3 master equipment share their live damage gate.
        pub fn tag_available(&self) -> Result<bool>;
        /// Historical selection; callers must separately check current visibility before firing.
        pub fn target_lock(&self) -> Option<TargetLock>;
        /// Inspect the single selected target without projecting it to a particular target kind.
        pub fn target_selection(&self) -> Option<TargetSelection>;
        /// Construction baseline used by the shared attacker movement calculation.
        pub fn template_speed(&self) -> f64;
        /// Pending recovery, including its saved countdown.
        pub fn unjam(&self) -> Option<Unjam>;
        /// Temporary conditions by mount index; existing recycle clocks govern recovery.
        pub fn weapon_failures(&self) -> &'a BTreeMap<usize, EquipmentFailure>;
        /// Whether a valid mount's feed is jammed; a jam does not destroy its critical slots.
        pub fn weapon_jammed(&self, index: usize) -> Result<bool>;
        /// Inspect functioning equipment, remaining matching salvos and recycle time.
        pub fn weapon_readiness(&self, index: usize) -> Result<WeaponReadiness>;
        /// Active recycle countdowns keyed by zero-based resolved weapon index.
        pub fn weapon_recycle(&self) -> &'a BTreeMap<usize, u16>;
    }

    shared_fields! {
        /// Whether another unit may tow this one.
        towable: bool;
        /// Whether the unit is dug in as a fortification.
        fortified: bool;
        /// Whether the unit is holding fire.
        weapons_hold: bool;
        /// Who the unit is shown to.
        visibility: Visibility;
        /// Whether the unit is exempt from combat.
        combat_safe: bool;
        /// The section the unit is aiming at, if any.
        aimed_section: Option<AimSelection>;
        /// Saved auxiliary cockpit preferences.
        auxiliary_preferences: auxiliary_preferences::AuxiliaryPreferences;
        /// Saved base movement overrides.
        base_movement_fields: base_movement_fields::BaseMovementFields;
        /// Shots fired and hits scored.
        shot_counters: shot_counters::ShotCounters;
        /// Damage dealt and taken.
        damage_counters: damage_counters::DamageCounters;
        /// Units this one has destroyed.
        units_killed: i32;
    }

    /// The identifier the unit's owner prefers on the battlefield.
    pub(super) fn preferred_id(&self) -> Option<&'a PreferredId> {
        match *self {
            Self::Mech(unit) => unit.preferred_id.as_ref(),
            Self::Vehicle(vehicle) => vehicle.preferred_id.as_ref(),
        }
    }

    /// Capitalized and lowercase words naming the chassis in messages.
    pub(super) fn nouns(&self) -> (&'static str, &'static str) {
        match self {
            Self::Mech(_) => ("Unit", "unit"),
            Self::Vehicle(_) => ("Vehicle", "vehicle"),
        }
    }

    /// Run the unit's own validation, reusing a result for an unchanged unit.
    pub(super) fn validate_local(&self, id: ObjectId) -> Result<()> {
        match self {
            Self::Mech(unit) => super::validation_context::unit(id, unit),
            Self::Vehicle(vehicle) => super::validation_context::vehicle(id, vehicle),
        }
    }

    /// Apply the rules for a unit moving under its own power.
    pub(super) fn validate_untowed(&self) -> Result<()> {
        match self {
            Self::Mech(unit) => unit.validate_untowed(),
            Self::Vehicle(vehicle) => vehicle.validate_untowed(),
        }
    }

    /// The identity summary the world keeps for the unit.
    pub(super) fn identity(&self) -> StoredBattleUnit {
        match self {
            Self::Mech(unit) => unit.identity(),
            Self::Vehicle(vehicle) => vehicle.identity(),
        }
    }
}

/// Mutable access to a Mech or vehicle through the operations both chassis share.
#[derive(Debug)]
pub enum UnitMut<'a> {
    /// A constructed BattleMech.
    Mech(&'a mut Mech),
    /// A combat vehicle.
    Vehicle(&'a mut Vehicle),
}

impl UnitMut<'_> {
    /// Read the same unit through its shared getters.
    pub fn as_ref(&self) -> UnitRef<'_> {
        match self {
            Self::Mech(unit) => UnitRef::Mech(unit),
            Self::Vehicle(vehicle) => UnitRef::Vehicle(vehicle),
        }
    }

    forward! { mut:
        /// Clear a feed jam after the caller has resolved recovery or repair; no automatic recovery is implied.
        pub fn clear_weapon_jam(&mut self, index: usize) -> Result<bool>;
        /// Record a feed jam inside the enclosing attack transaction without spending supply or heat.
        /// Returns whether this call introduced the jam.
        pub fn jam_weapon(&mut self, index: usize) -> Result<bool>;
        /// Forwards to the chassis's `set_administrative_attribute`.
        pub(super) fn set_administrative_attribute(&mut self, name: &str, value: impl ToString);
    }
}

/// A replacement construction template for either chassis.
#[derive(Debug, Clone)]
pub enum UnitDefinition {
    /// A BattleMech template.
    Mech(MechTemplate),
    /// A combat vehicle template.
    Vehicle(VehicleTemplate),
}

impl From<MechTemplate> for UnitDefinition {
    fn from(definition: MechTemplate) -> Self {
        Self::Mech(definition)
    }
}

impl From<VehicleTemplate> for UnitDefinition {
    fn from(definition: VehicleTemplate) -> Self {
        Self::Vehicle(definition)
    }
}

/// The draft copy an edit changes.
enum Draft {
    Mech(Box<Mech>),
    Vehicle(Box<Vehicle>),
}

/// A batch of edits to one unit's draft, validated together when the batch ends.
///
/// Edits that cannot apply, such as a bin the unit lacks, are reported by
/// [`BtechState::edit_unit`] and abandon the whole batch.
pub struct UnitEdit {
    draft: Draft,
    error: Option<anyhow::Error>,
}

impl UnitEdit {
    /// Record the first edit that could not apply.
    fn fail(&mut self, error: anyhow::Error) {
        self.error.get_or_insert(error);
    }

    /// Replace the unit's dice stream, for example with [`Dice::seeded`].
    pub fn set_dice(&mut self, dice: Dice) {
        match &mut self.draft {
            Draft::Mech(unit) => unit.dice = dice,
            Draft::Vehicle(vehicle) => vehicle.dice = dice,
        }
    }

    /// Replace the dice stream the unit's empty cockpit uses for crew recovery.
    pub fn set_crew_recovery_dice(&mut self, dice: Dice) {
        match &mut self.draft {
            Draft::Mech(unit) => unit.crew_recovery.set_dice(dice),
            Draft::Vehicle(vehicle) => vehicle.crew_recovery.set_dice(dice),
        }
    }

    /// Set the power state without running a startup or shutdown sequence.
    pub fn set_power(&mut self, power: Power) {
        match &mut self.draft {
            Draft::Mech(unit) => unit.power = power,
            Draft::Vehicle(vehicle) => vehicle.power = power,
        }
    }

    /// Replace the construction template. It must match the unit's chassis.
    pub fn set_definition(&mut self, definition: impl Into<UnitDefinition>) {
        match (&mut self.draft, definition.into()) {
            (Draft::Mech(unit), UnitDefinition::Mech(definition)) => {
                unit.set_fixture_definition(definition)
            }
            (Draft::Vehicle(vehicle), UnitDefinition::Vehicle(definition)) => {
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
    pub fn edit_motion(&mut self, edit: impl FnOnce(&mut Motion)) {
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
    pub fn unit(&self, id: ObjectId) -> Option<UnitRef<'_>> {
        if let Some(unit) = self.constructed.get(&id) {
            return Some(UnitRef::Mech(unit));
        }
        self.vehicles.get(&id).map(UnitRef::Vehicle)
    }

    /// Borrow a Mech or vehicle mutably through the operations both chassis share.
    pub fn unit_mut(&mut self, id: ObjectId) -> Option<UnitMut<'_>> {
        if self.constructed.contains_key(&id) {
            return self.constructed.get_mut(&id).map(UnitMut::Mech);
        }
        self.vehicles.get_mut(&id).map(UnitMut::Vehicle)
    }

    /// Apply a batch of edits to one Mech or vehicle and validate the result once.
    ///
    /// The edits change a draft copy, so the batch may pass through states that would be
    /// invalid on their own, such as cutting power before clearing the throttle. The draft
    /// replaces the unit only if every edit applied and the result passes the same
    /// validation a saved record must pass when the server loads it; otherwise the
    /// state is unchanged. Like a record rewrite, a committed edit clears the runtime-only
    /// state a serialization round trip drops.
    pub fn edit_unit(&mut self, id: ObjectId, edit: impl FnOnce(&mut UnitEdit)) -> Result<()> {
        let draft = if let Some(unit) = self.constructed.get(&id) {
            Draft::Mech(Box::new(unit.clone()))
        } else if let Some(vehicle) = self.vehicles.get(&id) {
            Draft::Vehicle(Box::new(vehicle.clone()))
        } else {
            bail!("#{} has no unit or vehicle record", id.0);
        };
        let mut batch = UnitEdit { draft, error: None };
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

    /// Replace a unit's dice stream. See [`UnitEdit::set_dice`].
    pub fn set_unit_dice(&mut self, id: ObjectId, dice: Dice) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_dice(dice))
    }

    /// Replace a unit's crew recovery dice. See [`UnitEdit::set_crew_recovery_dice`].
    pub fn set_unit_crew_recovery_dice(&mut self, id: ObjectId, dice: Dice) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_crew_recovery_dice(dice))
    }

    /// Set a unit's power state. See [`UnitEdit::set_power`].
    pub fn set_unit_power(&mut self, id: ObjectId, power: Power) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_power(power))
    }

    /// Replace a unit's construction template. See [`UnitEdit::set_definition`].
    pub fn set_unit_definition(
        &mut self,
        id: ObjectId,
        definition: impl Into<UnitDefinition>,
    ) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_definition(definition))
    }

    /// Replace a unit's ammunition. See [`UnitEdit::set_ammunition`].
    pub fn set_unit_ammunition(&mut self, id: ObjectId, rounds: Vec<u16>) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_ammunition(rounds))
    }

    /// Set one bin's rounds. See [`UnitEdit::set_ammunition_bin`].
    pub fn set_unit_ammunition_bin(&mut self, id: ObjectId, bin: usize, rounds: u16) -> Result<()> {
        self.edit_unit(id, |unit| unit.set_ammunition_bin(bin, rounds))
    }

    /// Edit a placed unit's motion. See [`UnitEdit::edit_motion`].
    pub fn edit_unit_motion(&mut self, id: ObjectId, edit: impl FnOnce(&mut Motion)) -> Result<()> {
        self.edit_unit(id, |unit| unit.edit_motion(edit))
    }

    /// Replace a player's consciousness recovery dice stream.
    pub fn set_recovery_dice(&mut self, player: ObjectId, dice: Dice) -> Result<()> {
        self.recoveries
            .get_mut(&player)
            .with_context(|| format!("#{} has no recovery record", player.0))?
            .set_dice(dice);
        self.clear_runtime_state();
        Ok(())
    }

    /// Replace a map's fire spread dice stream if it has one, reporting whether it did.
    /// A map without a stream keeps none, because whether one exists changes ignition.
    pub fn replace_map_fire_dice(&mut self, map: ObjectId, dice: Dice) -> Result<bool> {
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
    use crate::{Config, Kind, MapAsset, World};

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
            MechTemplate::parse("JR7-D", include_str!("../../game/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        crate::create_battle_vehicle(
            &mut world,
            vehicle,
            VehicleTemplate::parse(
                "Demolisher",
                include_str!("../../game/units/Demolisher.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        super::super::create_map(
            &mut world,
            map,
            "test",
            MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
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
            let dice = Dice::seeded([7; 32]);
            check(
                &|state| state.set_unit_dice(id, dice.clone()).unwrap(),
                &|record| record["dice"] = serde_json::to_value(&dice).unwrap(),
            );
            check(
                &|state| state.set_unit_crew_recovery_dice(id, dice.clone()).unwrap(),
                &|record| record["crew_recovery"]["dice"] = serde_json::to_value(&dice).unwrap(),
            );
            check(
                &|state| state.set_unit_power(id, Power::Running).unwrap(),
                &|record| {
                    record["power"] = serde_json::to_value(Power::Running).unwrap();
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
        assert!(world.btech.set_unit_power(map, Power::Off).is_err());
        assert!(world.btech.set_recovery_dice(mech, Dice::fresh()).is_err());
        assert_eq!(world.btech, before);
    }

    /// A batch validates once, so it may pass through states invalid on their own.
    #[test]
    fn a_batch_validates_only_its_result() {
        let (mut world, _, vehicle, _) = world();
        world
            .btech
            .edit_unit(vehicle, |unit| {
                unit.set_power(Power::Running);
                unit.edit_motion(|motion| motion.desired_speed = 10.0);
            })
            .unwrap();
        // Cutting power first leaves a throttle on an inactive vehicle, which is invalid.
        let mut stepwise = world.btech.clone();
        assert!(stepwise.set_unit_power(vehicle, Power::Off).is_err());
        world
            .btech
            .edit_unit(vehicle, |unit| {
                unit.set_power(Power::Off);
                unit.edit_motion(|motion| motion.desired_speed = 0.0);
            })
            .unwrap();
        let unit = world.btech.unit(vehicle).unwrap();
        assert_eq!(unit.power(), Power::Off);
        assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
    }

    /// A unit without power may keep a heading order but not a throttle, on either chassis.
    #[test]
    fn unpowered_units_keep_a_pending_turn_but_no_throttle() {
        let (mut world, mech, vehicle, _) = world();
        for id in [mech, vehicle] {
            world
                .btech
                .edit_unit_motion(id, |motion| motion.desired_heading = 90.0)
                .unwrap();
            let unit = world.btech.unit(id).unwrap();
            assert_eq!(unit.power(), Power::Off);
            assert_eq!(unit.motion().unwrap().desired_heading, 90.0);
            assert!(
                world
                    .btech
                    .edit_unit_motion(id, |motion| motion.desired_speed = 10.0)
                    .is_err()
            );
        }
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
