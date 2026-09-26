//! Pilot-selected unit preferences, persisted with the unit rather than the cockpit occupant.
use crate::{ObjectId, World};
use anyhow::Result;

/// Set downhill cliff behavior on the current pilot's unit.
pub fn set_auto_fall(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    enabled: bool,
) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        super::vehicle_power::controlled(world, id, pilot)?;
        world.btech.vehicles.get_mut(&id).unwrap().auto_fall = enabled;
        return Ok(());
    }
    preference_access(world, id, pilot)?;
    world.btech.constructed.get_mut(&id).unwrap().auto_fall = enabled;
    Ok(())
}

/// Query and mutation use the same cockpit checks.
pub(crate) fn preference_access(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    super::power::controlled_unit(world, id, pilot)
}

/// Enable or disable external searchlight transition warnings for this cockpit.
pub fn set_searchlight_warning(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    enabled: bool,
) -> Result<()> {
    notice_preference_access(world, id, pilot)?;
    *notice_preferences(world, id).0 = enabled;
    Ok(())
}

impl super::BattleUnit {
    /// Whether armor threshold changes notify the occupants.
    pub fn armor_warning(&self) -> bool {
        !self.no_armor_warning
    }
    /// Whether low ammunition notifies the occupants.
    pub fn ammunition_warning(&self) -> bool {
        !self.no_ammunition_warning
    }
}
/// Set the assigned pilot's armor warning preference.
pub fn set_armor_warning(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    enabled: bool,
) -> Result<()> {
    notice_preference_access(world, id, pilot)?;
    *combat_preferences(world, id).0 = !enabled;
    Ok(())
}
/// Set the assigned pilot's ammunition warning preference.
pub fn set_ammunition_warning(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    enabled: bool,
) -> Result<()> {
    notice_preference_access(world, id, pilot)?;
    *combat_preferences(world, id).1 = !enabled;
    Ok(())
}

impl super::BattleUnit {
    /// Whether the pilot has enabled friendly-fire protection.
    pub fn friendly_fire_safety(&self) -> bool {
        self.friendly_fire_safety
    }
}

impl super::BattleVehicle {
    /// Whether a piloted vehicle skips downhill cliff avoidance.
    pub fn auto_fall(&self) -> bool {
        self.auto_fall
    }

    /// Whether the pilot has enabled friendly-fire protection.
    pub fn friendly_fire_safety(&self) -> bool {
        self.friendly_fire_safety
    }
}

/// Vehicle preferences with implemented movement and firing consumers.
pub(crate) fn vehicle_catalog(unit: &super::BattleVehicle) -> Vec<Preference> {
    vec![
        mw_safety_preference(unit.mw_safety()),
        bth_debug_preference(unit.bth_debug()),
        auto_fall_preference(unit.auto_fall()),
        friendly_fire_preference(unit.friendly_fire_safety()),
        searchlight_preference(unit.searchlight_warning()),
        autocon_preference(unit.autocon_shutdown()),
        armor_preference(unit.armor_warning()),
        ammunition_preference(unit.ammunition_warning()),
    ]
}

/// Set the assigned pilot's friendly-fire safety; coolant remains exempt.
pub fn set_friendly_fire_safety(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    enabled: bool,
) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        super::vehicle_power::controlled(world, id, pilot)?;
        world
            .btech
            .vehicles
            .get_mut(&id)
            .unwrap()
            .friendly_fire_safety = enabled;
        return Ok(());
    }
    preference_access(world, id, pilot)?;
    world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .friendly_fire_safety = enabled;
    Ok(())
}

/// One supported cockpit preference, including its current value and mutation boundary.
pub(crate) struct Preference {
    pub name: &'static str,
    pub enabled: bool,
    pub set: fn(&mut World, ObjectId, ObjectId, bool) -> Result<()>,
    pub message: &'static str,
}

/// Keep preference listing, toggling and explicit settings on one command path.
pub(crate) fn catalog(unit: &super::BattleUnit) -> [Preference; 8] {
    [
        mw_safety_preference(unit.mw_safety()),
        bth_debug_preference(unit.bth_debug()),
        auto_fall_preference(unit.auto_fall()),
        searchlight_preference(unit.searchlight_warning()),
        armor_preference(unit.armor_warning()),
        ammunition_preference(unit.ammunition_warning()),
        autocon_preference(unit.autocon_shutdown()),
        friendly_fire_preference(unit.friendly_fire_safety()),
    ]
}

impl super::BattleUnit {
    /// Whether routine contact notices include targets whose reactors are not running.
    pub fn autocon_shutdown(&self) -> bool {
        self.autocon_shutdown
    }
}

/// Change shutdown-contact notice policy without changing acquisition or saved list categories.
pub fn set_autocon_shutdown(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    enabled: bool,
) -> Result<()> {
    notice_preference_access(world, id, pilot)?;
    *notice_preferences(world, id).1 = enabled;
    Ok(())
}

/// Shared metadata keeps vehicle and Mech command feedback identical.
fn auto_fall_preference(enabled: bool) -> Preference {
    Preference {
        name: "AutoFall",
        enabled,
        set: set_auto_fall,
        message: "Suicidal jumps off cliffs toggled",
    }
}

/// Shared metadata for teammate protection on either chassis.
fn friendly_fire_preference(enabled: bool) -> Preference {
    Preference {
        name: "FFSafety",
        enabled,
        set: set_friendly_fire_safety,
        message: "Friendly Fire Safeties flipped",
    }
}

/// Select the shared mutable notice preferences after cockpit admission.
fn notice_preferences(world: &mut World, id: ObjectId) -> (&mut bool, &mut bool) {
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        return (&mut unit.searchlight_warning, &mut unit.autocon_shutdown);
    }
    let unit = world.btech.vehicles.get_mut(&id).expect("admitted vehicle");
    (&mut unit.searchlight_warning, &mut unit.autocon_shutdown)
}

impl super::BattleVehicle {
    /// Whether external illumination transitions notify the cockpit.
    pub fn searchlight_warning(&self) -> bool {
        self.searchlight_warning
    }

    /// Whether routine contact notices include shutdown units.
    pub fn autocon_shutdown(&self) -> bool {
        self.autocon_shutdown
    }
}

/// Shared searchlight-warning metadata for every supported chassis.
fn searchlight_preference(enabled: bool) -> Preference {
    Preference {
        name: "SLWarn",
        enabled,
        set: set_searchlight_warning,
        message: "The warning when lit by searchlight is now",
    }
}

/// Shared shutdown-contact metadata for every supported chassis.
fn autocon_preference(enabled: bool) -> Preference {
    Preference {
        name: "AutoconShutdown",
        enabled,
        set: set_autocon_shutdown,
        message: "Autocon on shutdown units turned",
    }
}

/// Admit notice settings on either chassis without widening anatomy-specific preference setters.
fn notice_preference_access(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_power::controlled(world, id, pilot);
    }
    preference_access(world, id, pilot)
}

/// Borrow combat-warning preferences after shared cockpit admission.
fn combat_preferences(world: &mut World, id: ObjectId) -> (&mut bool, &mut bool) {
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        return (&mut unit.no_armor_warning, &mut unit.no_ammunition_warning);
    }
    let unit = world.btech.vehicles.get_mut(&id).expect("admitted vehicle");
    (&mut unit.no_armor_warning, &mut unit.no_ammunition_warning)
}

impl super::BattleVehicle {
    /// Whether armor severity transitions notify the cockpit.
    pub fn armor_warning(&self) -> bool {
        !self.no_armor_warning
    }
    /// Whether firing warns about low installed ammunition inventory.
    pub fn ammunition_warning(&self) -> bool {
        !self.no_ammunition_warning
    }
}

/// Armor-warning command metadata is shared across chassis.
fn armor_preference(enabled: bool) -> Preference {
    Preference {
        name: "ArmorWarn",
        enabled,
        set: set_armor_warning,
        message: "Low-armor warnings turned",
    }
}

/// Ammunition-warning command metadata is shared across chassis.
fn ammunition_preference(enabled: bool) -> Preference {
    Preference {
        name: "AmmoWarn",
        enabled,
        set: set_ammunition_warning,
        message: "Warning when running out of Ammunition switched",
    }
}

/// BTHDebug shares the reference's configuration-only toggle on every chassis.
fn bth_debug_preference(enabled: bool) -> Preference {
    Preference {
        name: "BTHDebug",
        enabled,
        set: super::set_bth_debug,
        message: "BTH Debugging is now",
    }
}

/// Inverted safety metadata uses a positive ON/OFF value in cockpit controls.
fn mw_safety_preference(enabled: bool) -> Preference {
    Preference {
        name: "MWSafety",
        enabled,
        set: super::set_mw_safety,
        message: "MechWarrior Safeties flipped",
    }
}
