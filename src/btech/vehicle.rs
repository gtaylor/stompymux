//! Shared vehicle and rotorcraft material state with validated replay and explicit damage phases.
use super::{
    DamagePhase, DamageResult, SectionState, VehicleLoadout, VehicleSection, VehicleTemplate,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Vehicle protection and ammunition, independent of Mech anatomy and world admission.
/// Combat adapters must still resolve hit locations, criticals, crew effects and notifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "VehicleRecord")]
pub struct Vehicle {
    #[serde(default)]
    pub(super) auxiliary_preferences: super::auxiliary_preferences::AuxiliaryPreferences,
    #[serde(default)]
    pub(super) base_movement_fields: super::base_movement_fields::BaseMovementFields,
    #[serde(default)]
    pub(super) shot_counters: super::shot_counters::ShotCounters,
    #[serde(default)]
    pub(super) damage_counters: super::damage_counters::DamageCounters,
    /// Attributed destruction events, independent of damage volume.
    #[serde(default)]
    pub(super) units_killed: i32,
    pub(super) no_armor_warning: bool,
    pub(super) no_ammunition_warning: bool,
    pub(super) searchlight: super::Searchlight,
    pub(super) autocon_shutdown: bool,
    pub(super) searchlight_warning: bool,
    pub(super) illumination_observed: bool,
    pub(super) display_name: super::display_name::DisplayName,
    #[serde(default)]
    pub(super) markings: super::markings::Markings,
    #[serde(default)]
    pub(super) sixth_sense: super::sixth_sense::SixthSense,
    #[serde(default)]
    pub(super) last_startup: i64,
    pub(super) cockpit_links: super::cockpit_links::CockpitLinks,
    pub(super) hardware: super::hardware_settings::HardwareSettings,
    #[serde(default)]
    pub(super) self_destruct_safe: bool,
    pub(super) self_destruct: Option<super::SelfDestruct>,
    /// Elapsed one-second camouflage preparation checks.
    pub(super) hide_elapsed: Option<u16>,
    /// Scenario permission shared with Mechs for out-of-character pickup targets.
    pub(super) towable: bool,
    /// Scenario emplacement; movement and towing share the fortification gate.
    #[serde(default)]
    pub(super) fortified: bool,
    /// Administrator-assigned observer role.
    pub(super) observer: bool,
    /// Operator-imposed firing restriction, separate from weapon mechanics.
    #[serde(default)]
    pub(super) weapons_hold: bool,
    /// Operator-imposed immunity to combat damage.
    #[serde(default)]
    pub(super) combat_safe: bool,
    /// Operator visibility privileges, independent of sensor equipment.
    #[serde(default)]
    pub(super) visibility: super::Visibility,
    pub(super) dig: super::DigState,
    #[serde(default)]
    contract_loadout: bool,
    #[serde(default)]
    administrative_raw: Option<super::AdministrativeRawUnit>,
    definition: VehicleTemplate,
    pub(super) sections: BTreeMap<VehicleSection, SectionState>,
    pub(super) ammunition: Vec<u16>,
    position: Option<super::Position>,
    /// Keep the physical pose after tactical membership is removed.
    #[serde(default)]
    pub(super) detached: bool,
    map_slot: Option<u32>,
    #[serde(default)]
    pub(super) battlefield_label: Option<String>,
    /// Optional configured identity, used only when selecting a new battlefield ID.
    #[serde(default)]
    pub(super) preferred_id: Option<super::PreferredId>,
    pub(super) pilot: Option<crate::ObjectId>,
    pub(super) power: super::Power,
    pub(super) motion: Option<super::Motion>,
    /// Facing retained while the vehicle has no map motion, as in the native Mech record.
    #[serde(default)]
    pub(super) detached_heading: f64,
    pub(super) under_bridge: bool,
    pub(super) ground_elevation: Option<f64>,
    /// Ground chassis use the same forced-descent clock as aircraft after losing support.
    pub(super) free_fall: Option<super::FreeFall>,
    /// Ground chassis share the Mech cocoon and jump-jet descent model.
    #[serde(default)]
    pub(super) orbital_drop: Option<super::OrbitalDrop>,
    #[serde(default)]
    pub(super) propulsion: super::propulsion::Propulsion,
    #[serde(default)]
    pub(super) live_mass: super::live_mass::LiveMass,
    #[serde(default)]
    pub(super) critical_conditions: super::critical_conditions::CriticalConditions,
    motive_speed_loss: f64,
    immobilized: bool,
    pub(super) turret_offset: f64,
    pub(super) automatic_turret: bool,
    turret_locked: bool,
    pub(super) turret_jammed: bool,
    pub(super) turret_repairs: Vec<u8>,
    pub(super) crew_stun_remaining: u8,
    #[serde(default)]
    pub(super) crew_stun_condition: Option<bool>,
    pub(super) pilot_injuries: u8,
    /// Confirmed tactical pilot death, independently of the current injury count.
    pub(super) pilot_killed: bool,
    pub(super) character_pilot: Option<super::CharacterPilotStatus>,
    /// Permanent water destruction, independent of hull integrity and crew injury.
    flooded: bool,
    /// Sections whose equipment was disabled by vacuum exposure.
    #[serde(default)]
    pub(super) breached_sections: std::collections::BTreeSet<VehicleSection>,
    /// Hull disabled by destruction of its carrier, independently of crew injury.
    transport_destroyed: bool,
    /// Tail rotor loss constrains flight controls without destroying the hull.
    pub(super) tail_rotor_destroyed: bool,
    pub(super) vtol_fuel: Option<super::VtolFuel>,
    pub(super) vtol_flight: Option<super::VtolFlight>,
    /// Instant loss of the entire crew, independent of accumulated pilot injuries.
    crew_killed: bool,
    pub(super) crew_recovery: super::Recovery,
    pub(super) piloting_damage: u8,
    pub(super) gunnery_damage: u8,
    pub(super) lost_stabilizers: std::collections::BTreeSet<VehicleSection>,
    pub(super) lost_criticals: std::collections::BTreeSet<super::VehicleCriticalLocation>,
    pub(super) brief: super::BriefSettings,
    pub(super) tics: super::Tics,
    pub(super) signature: super::UnitSignature,
    pub(super) scanner_perception: i16,
    pub(super) radio: [super::RadioChannel; 16],
    pub(super) radio_skill: i16,
    pub(super) fired_recently: bool,
    pub(super) radio_experience_remaining: u8,
    pub(super) experience: super::UnitExperience,
    pub(super) friendly_fire_safety: bool,
    pub(super) auto_fall: bool,
    pub(super) ams_enabled: bool,
    /// Passive weapon heat and coolant credit; ground vehicles do not sample overheating.
    pub(super) weapon_heat: f64,
    pub(super) inferno_remaining: u32,
    pub(super) burning_sections: BTreeMap<VehicleSection, u8>,
    pub(super) extinguishing: Option<u8>,
    pub(super) building_entry: Option<super::BuildingEntry>,
    pub(super) beacons: BTreeMap<VehicleSection, std::collections::BTreeSet<super::BeaconKind>>,
    pub(super) target_lock: Option<super::TargetSelection>,
    #[serde(default)]
    pub(super) aimed_section: Option<super::AimSelection>,
    pub(super) artillery_adjustment: u8,
    pub(super) c3_network: Option<u64>,
    pub(super) c3i_network: Option<u64>,
    /// Which network families the server may link automatically.
    pub(super) network_automation: super::NetworkAutomation,
    pub(super) electronics: super::Electronics,
    pub(super) spotter: Option<crate::ObjectId>,
    pub(super) spotter_events: super::SpotterEvents,
    pub(super) tag: super::TagState,
    pub(super) contacts: BTreeMap<crate::ObjectId, super::Contact>,
    pub(super) fire_modes: BTreeMap<usize, super::FireMode>,
    pub(super) ammunition_modes: BTreeMap<usize, super::AmmunitionMode>,
    pub(super) ammunition_sections: BTreeMap<usize, VehicleSection>,
    pub(super) unjam: Option<super::Unjam>,
    pub(super) pod_removal: Option<u8>,
    pub(super) jammed_weapons: std::collections::BTreeSet<usize>,
    #[serde(default)]
    pub(super) component_failures: Vec<super::ComponentFailure<super::VehicleCriticalLocation>>,
    pub(super) weapon_failures: BTreeMap<usize, super::EquipmentFailure>,
    pub(super) weapon_recycle: BTreeMap<usize, u16>,
    /// Gauss mounts deliberately powered down; independent of material damage.
    #[serde(default)]
    pub(super) powered_down_weapons: std::collections::BTreeSet<usize>,
    pub(super) spent_launchers: std::collections::BTreeSet<usize>,
    /// Replayable outcomes owned by this vehicle and committed with its effects.
    pub(super) dice: super::Dice,
}

/// Snapshot input is validated against its owned definition before becoming domain state.
#[derive(Deserialize)]
struct VehicleRecord {
    #[serde(default)]
    contract_loadout: bool,
    #[serde(default)]
    administrative_raw: Option<super::AdministrativeRawUnit>,
    #[serde(default)]
    auxiliary_preferences: super::auxiliary_preferences::AuxiliaryPreferences,
    #[serde(default)]
    base_movement_fields: super::base_movement_fields::BaseMovementFields,
    #[serde(default)]
    shot_counters: super::shot_counters::ShotCounters,
    #[serde(default)]
    damage_counters: super::damage_counters::DamageCounters,
    #[serde(default)]
    units_killed: i32,
    #[serde(default)]
    no_armor_warning: bool,
    #[serde(default)]
    no_ammunition_warning: bool,
    #[serde(default)]
    searchlight: super::Searchlight,
    #[serde(default)]
    autocon_shutdown: bool,
    #[serde(default)]
    searchlight_warning: bool,
    #[serde(default)]
    illumination_observed: bool,
    #[serde(default)]
    display_name: super::display_name::DisplayName,
    #[serde(default)]
    markings: super::markings::Markings,
    #[serde(default)]
    sixth_sense: super::sixth_sense::SixthSense,
    #[serde(default)]
    last_startup: i64,
    #[serde(default)]
    cockpit_links: super::cockpit_links::CockpitLinks,
    #[serde(default)]
    hardware: super::hardware_settings::HardwareSettings,
    #[serde(default)]
    self_destruct_safe: bool,
    #[serde(default)]
    self_destruct: Option<super::SelfDestruct>,
    #[serde(default)]
    hide_elapsed: Option<u16>,
    #[serde(default)]
    towable: bool,
    #[serde(default)]
    fortified: bool,
    #[serde(default)]
    observer: bool,
    #[serde(default)]
    weapons_hold: bool,
    #[serde(default)]
    combat_safe: bool,
    #[serde(default)]
    visibility: super::Visibility,
    #[serde(default)]
    dig: super::DigState,
    definition: VehicleTemplate,
    sections: BTreeMap<VehicleSection, SectionState>,
    ammunition: Vec<u16>,
    position: Option<super::Position>,
    /// Keep the physical pose after tactical membership is removed.
    #[serde(default)]
    pub(super) detached: bool,
    map_slot: Option<u32>,
    #[serde(default)]
    battlefield_label: Option<String>,
    #[serde(default)]
    preferred_id: Option<super::PreferredId>,
    pilot: Option<crate::ObjectId>,
    power: super::Power,
    motion: Option<super::Motion>,
    #[serde(default)]
    detached_heading: f64,
    under_bridge: bool,
    #[serde(default)]
    ground_elevation: Option<f64>,
    #[serde(default)]
    free_fall: Option<super::FreeFall>,
    #[serde(default)]
    orbital_drop: Option<super::OrbitalDrop>,
    #[serde(default)]
    pub(super) propulsion: super::propulsion::Propulsion,
    #[serde(default)]
    pub(super) live_mass: super::live_mass::LiveMass,
    #[serde(default)]
    pub(super) critical_conditions: super::critical_conditions::CriticalConditions,
    motive_speed_loss: f64,
    immobilized: bool,
    turret_offset: f64,
    #[serde(default)]
    automatic_turret: bool,
    turret_locked: bool,
    turret_jammed: bool,
    #[serde(default)]
    turret_repairs: Vec<u8>,
    #[serde(default)]
    crew_stun_remaining: u8,
    #[serde(default)]
    crew_stun_condition: Option<bool>,
    pilot_injuries: u8,
    #[serde(default)]
    pilot_killed: bool,
    character_pilot: Option<super::CharacterPilotStatus>,
    flooded: bool,
    #[serde(default)]
    breached_sections: std::collections::BTreeSet<VehicleSection>,
    /// Hull disabled by destruction of its carrier, independently of crew injury.
    transport_destroyed: bool,
    /// Tail rotor loss constrains flight controls without destroying the hull.
    tail_rotor_destroyed: bool,
    #[serde(default)]
    vtol_fuel: Option<super::VtolFuel>,
    #[serde(default)]
    vtol_flight: Option<super::VtolFlight>,
    /// Instant loss of the entire crew, independent of accumulated pilot injuries.
    crew_killed: bool,
    crew_recovery: super::Recovery,
    piloting_damage: u8,
    gunnery_damage: u8,
    lost_stabilizers: std::collections::BTreeSet<VehicleSection>,
    lost_criticals: std::collections::BTreeSet<super::VehicleCriticalLocation>,
    brief: super::BriefSettings,
    #[serde(default)]
    tics: super::Tics,
    signature: super::UnitSignature,
    scanner_perception: i16,
    #[serde(default)]
    radio: [super::RadioChannel; 16],
    #[serde(default = "super::radio::default_skill")]
    radio_skill: i16,
    #[serde(default)]
    fired_recently: bool,
    #[serde(default)]
    radio_experience_remaining: u8,
    experience: super::UnitExperience,
    friendly_fire_safety: bool,
    auto_fall: bool,
    ams_enabled: bool,
    #[serde(default)]
    weapon_heat: f64,
    #[serde(default)]
    inferno_remaining: u32,
    burning_sections: BTreeMap<VehicleSection, u8>,
    #[serde(default)]
    extinguishing: Option<u8>,
    #[serde(default)]
    building_entry: Option<super::BuildingEntry>,
    beacons: BTreeMap<VehicleSection, std::collections::BTreeSet<super::BeaconKind>>,
    target_lock: Option<super::TargetSelection>,
    #[serde(default)]
    aimed_section: Option<super::AimSelection>,
    artillery_adjustment: u8,
    c3_network: Option<u64>,
    c3i_network: Option<u64>,
    #[serde(default)]
    network_automation: super::NetworkAutomation,
    electronics: super::Electronics,
    spotter: Option<crate::ObjectId>,
    #[serde(default)]
    spotter_events: super::SpotterEvents,
    #[serde(default)]
    tag: super::TagState,
    contacts: BTreeMap<crate::ObjectId, super::Contact>,
    fire_modes: BTreeMap<usize, super::FireMode>,
    ammunition_modes: BTreeMap<usize, super::AmmunitionMode>,
    #[serde(default)]
    ammunition_sections: BTreeMap<usize, VehicleSection>,
    #[serde(default)]
    unjam: Option<super::Unjam>,
    #[serde(default)]
    pod_removal: Option<u8>,
    jammed_weapons: std::collections::BTreeSet<usize>,
    #[serde(default)]
    component_failures: Vec<super::ComponentFailure<super::VehicleCriticalLocation>>,
    weapon_failures: BTreeMap<usize, super::EquipmentFailure>,
    weapon_recycle: BTreeMap<usize, u16>,
    #[serde(default)]
    powered_down_weapons: std::collections::BTreeSet<usize>,
    spent_launchers: std::collections::BTreeSet<usize>,
    dice: super::Dice,
}

// Live fields change routinely while a vehicle moves, fires and recovers; everything
// else is construction, damage and settings, saved only when it changes.
super::saved_parts::saved_parts!(Vehicle {
    core: [
        auxiliary_preferences,
        base_movement_fields,
        units_killed,
        no_armor_warning,
        no_ammunition_warning,
        searchlight,
        autocon_shutdown,
        searchlight_warning,
        illumination_observed,
        display_name,
        markings,
        sixth_sense,
        cockpit_links,
        hardware,
        self_destruct_safe,
        self_destruct,
        towable,
        fortified,
        observer,
        weapons_hold,
        combat_safe,
        visibility,
        contract_loadout,
        administrative_raw,
        definition,
        sections,
        detached,
        map_slot,
        battlefield_label,
        preferred_id,
        pilot,
        under_bridge,
        propulsion,
        live_mass,
        critical_conditions,
        motive_speed_loss,
        immobilized,
        automatic_turret,
        turret_locked,
        turret_jammed,
        crew_stun_condition,
        pilot_injuries,
        pilot_killed,
        character_pilot,
        flooded,
        breached_sections,
        transport_destroyed,
        tail_rotor_destroyed,
        crew_killed,
        crew_recovery,
        piloting_damage,
        gunnery_damage,
        lost_stabilizers,
        lost_criticals,
        brief,
        tics,
        signature,
        scanner_perception,
        radio,
        radio_skill,
        radio_experience_remaining,
        experience,
        friendly_fire_safety,
        auto_fall,
        ams_enabled,
        burning_sections,
        building_entry,
        beacons,
        artillery_adjustment,
        c3_network,
        c3i_network,
        network_automation,
        electronics,
        spotter,
        tag,
        contacts,
        fire_modes,
        ammunition_modes,
        ammunition_sections,
        jammed_weapons,
        component_failures,
        weapon_failures,
        powered_down_weapons,
        spent_launchers,
    ],
    live: [
        detached_heading,
        turret_repairs,
        aimed_section,
        weapon_heat,
        fired_recently,
        shot_counters,
        damage_counters,
        last_startup,
        crew_stun_remaining,
        vtol_flight,
        vtol_fuel,
        free_fall,
        orbital_drop,
        ground_elevation,
        hide_elapsed,
        dig,
        inferno_remaining,
        extinguishing,
        unjam,
        pod_removal,
        spotter_events,
    ],
    live_always: [
        motion,
        position,
        power,
        target_lock,
        weapon_recycle,
        turret_offset,
        dice,
        ammunition,
    ],
});

impl Vehicle {
    /// The name of one of this vehicle's sections.
    pub fn section_name(&self, section: VehicleSection) -> &'static str {
        section.name()
    }

    /// World-level rules for a vehicle nothing is towing: it moves only under power and
    /// only while it still can.
    pub(super) fn validate_untowed(&self) -> Result<()> {
        let Some(motion) = self.motion() else {
            return Ok(());
        };
        motion.validate(
            super::speed_bonus::saved_limit(self.maximum_speed(), false, false, false) + 10.75,
        )?;
        ensure!(
            self.power() == super::Power::Running
                || !motion.translating()
                || self.idle_flight_controls(),
            "Inactive untowed vehicle retains motion"
        );
        ensure!(
            (self.maximum_speed() > 0.0 && !self.rotor_destroyed()) || !motion.active(),
            "Immobile untowed vehicle retains motion"
        );
        Ok(())
    }

    /// Replace the construction template in place, for fixtures that edit it.
    pub(super) fn set_fixture_definition(&mut self, definition: VehicleTemplate) {
        self.definition = definition;
    }

    pub(crate) fn administrative_raw(&self) -> Option<&super::AdministrativeRawUnit> {
        self.administrative_raw.as_ref()
    }

    pub(crate) fn administrative_raw_mut(
        &mut self,
        class: super::RawUnitClass,
        movement: super::RawMovement,
    ) -> &mut super::AdministrativeRawUnit {
        self.administrative_raw
            .get_or_insert_with(|| super::AdministrativeRawUnit::new(class, movement))
    }
    pub(super) fn set_contract_weapon_mode_names(
        &mut self,
        index: usize,
        fire: Vec<String>,
        ammunition: Vec<String>,
    ) -> Result<()> {
        let first = self
            .loadout()?
            .weapons
            .get(index)
            .and_then(|mount| mount.criticals.first())
            .copied()
            .context("weapon number is not mounted")?;
        if fire.iter().any(|mode| mode == "Destroyed") {
            self.lost_criticals.insert(first);
        } else {
            self.lost_criticals.remove(&first);
        }
        let failure = if fire.iter().any(|mode| mode == "Disabled") {
            Some(super::EquipmentFailure::Disabled)
        } else if fire.iter().any(|mode| mode == "Broken") {
            Some(super::EquipmentFailure::Dud)
        } else {
            None
        };
        if let Some(failure) = failure {
            self.weapon_failures.insert(index, failure);
        } else {
            self.weapon_failures.remove(&index);
        }
        if fire
            .iter()
            .any(|mode| matches!(mode.as_str(), "OneShot_Used" | "RocketFired"))
        {
            self.spent_launchers.insert(index);
        } else {
            self.spent_launchers.remove(&index);
        }
        let modes = &mut self
            .definition
            .sections
            .get_mut(&first.section)
            .expect("validated weapon section")
            .criticals
            .get_mut(&first.slot)
            .expect("validated weapon slot")
            .modes;
        super::unit::replace_critical_modes(modes, fire, ammunition);
        Ok(())
    }

    pub(super) fn replace_construction_contract(
        &mut self,
        definition: VehicleTemplate,
        touched: &[super::VehicleCriticalLocation],
    ) -> Result<()> {
        self.replace_construction_mode(definition, touched, true)
    }

    fn replace_construction_mode(
        &mut self,
        definition: VehicleTemplate,
        touched: &[super::VehicleCriticalLocation],
        contract: bool,
    ) -> Result<()> {
        let old = self.loadout()?.clone();
        let replacement = if contract {
            Self::new_contract(definition)?
        } else {
            Self::new(definition)?
        };
        let new = replacement.loadout()?.clone();
        let remap: Vec<_> = new
            .weapons
            .iter()
            .map(|mount| {
                old.weapons.iter().position(|prior| {
                    prior.weapon == mount.weapon
                        && prior.criticals == mount.criticals
                        && !mount.criticals.iter().any(|slot| touched.contains(slot))
                })
            })
            .collect();
        let ammunition = new
            .ammunition
            .iter()
            .map(|bin| {
                old.ammunition
                    .iter()
                    .position(|prior| {
                        prior.location == bin.location
                            && prior.weapon == bin.weapon
                            && !touched.contains(&bin.location)
                    })
                    .map_or(bin.rounds, |old| self.ammunition[old].min(bin.capacity))
            })
            .collect();
        self.definition = replacement.definition;
        self.contract_loadout = replacement.contract_loadout;
        self.ammunition = ammunition;
        self.fire_modes = super::unit::remap_map(&remap, &self.fire_modes);
        self.ammunition_modes = super::unit::remap_map(&remap, &self.ammunition_modes);
        self.ammunition_sections = super::unit::remap_map(&remap, &self.ammunition_sections);
        self.weapon_recycle = super::unit::remap_map(&remap, &self.weapon_recycle);
        self.jammed_weapons = super::unit::remap_set(&remap, &self.jammed_weapons);
        self.spent_launchers = super::unit::remap_set(&remap, &self.spent_launchers);
        self.powered_down_weapons = super::unit::remap_set(&remap, &self.powered_down_weapons);
        self.weapon_failures = super::unit::remap_map(&remap, &self.weapon_failures);
        self.lost_criticals.retain(|slot| !touched.contains(slot));
        self.component_failures
            .retain(|failure| !touched.contains(&failure.location));
        self.live_mass.invalidate();
        Ok(())
    }

    pub(super) fn set_administrative_heat_sinks(&mut self, count: u16) {
        self.definition.heat_sinks = Some(count);
    }

    pub(super) fn set_administrative_special(
        &mut self,
        attribute: &str,
        flag: &str,
        enabled: bool,
    ) {
        super::edit_special(&mut self.definition.attributes, attribute, flag, enabled);
    }

    pub(super) fn set_administrative_attribute(&mut self, name: &str, value: impl ToString) {
        self.definition
            .attributes
            .insert(name.into(), value.to_string());
    }

    pub(super) fn administrative_attribute(&self, name: &str) -> Option<&str> {
        self.definition.attributes.get(name).map(String::as_str)
    }

    pub(super) fn set_administrative_armor(
        &mut self,
        section: VehicleSection,
        armor: Option<u16>,
        internal: Option<u16>,
        rear: Option<u16>,
    ) {
        let original = self
            .definition
            .sections
            .get_mut(&section)
            .expect("validated section");
        let current = self.sections.get_mut(&section).expect("validated section");
        if let Some(value) = armor {
            original.armor = value;
            current.armor = value;
        }
        if let Some(value) = internal {
            original.internal = value;
            current.internal = value;
        }
        if let Some(value) = rear {
            original.rear = value;
            current.rear = value;
        }
    }

    pub(super) fn apply_immediate_repair(
        &mut self,
        section: VehicleSection,
        kind: super::AdministrativeRepairKind,
        value: u16,
        hull: super::ReattachHull,
    ) -> Result<()> {
        if kind == super::AdministrativeRepairKind::Part {
            self.repair_critical(super::VehicleCriticalLocation {
                section,
                slot: value as u8,
            })?;
            return Ok(());
        }
        let current = self.sections.get_mut(&section).expect("validated section");
        match kind {
            super::AdministrativeRepairKind::Armor => current.armor = value,
            super::AdministrativeRepairKind::Internal => current.internal = value,
            super::AdministrativeRepairKind::RearArmor => current.rear = value,
            super::AdministrativeRepairKind::Reattach => {
                // C mech_re_attach (mech_maintenance.c:501-514) restores only
                // sections reading as destroyed; aerospace hulls reset to one.
                if hull.destroyed(current.armor, current.internal) {
                    current.internal = if hull == super::ReattachHull::Aerospace {
                        1
                    } else {
                        self.definition.sections[&section].internal
                    };
                }
            }
            super::AdministrativeRepairKind::Part => unreachable!(),
        }
        Ok(())
    }

    fn repair_critical(&mut self, location: super::VehicleCriticalLocation) -> Result<()> {
        let loadout = self.loadout()?.clone();
        let weapon = loadout
            .weapons
            .iter()
            .position(|mount| mount.criticals.contains(&location));
        let ammunition = loadout
            .ammunition
            .iter()
            .position(|bin| bin.location == location);
        let critical = self
            .definition
            .sections
            .get_mut(&location.section)
            .and_then(|section| section.criticals.get_mut(&location.slot));
        // C mech_repair_part (mech_maintenance.c:450-489) restores whatever the
        // slot holds; empty slots clear already-clear bits and succeed silently.
        let Some(critical) = critical else {
            return Ok(());
        };
        critical.modes.retain(|mode| {
            !matches!(
                mode.as_str(),
                "Destroyed"
                    | "Disabled"
                    | "Broken"
                    | "Damaged"
                    | "OneShot_Used"
                    | "Jettisoned"
                    | "RocketFired"
            )
        });
        if weapon.is_some() || ammunition.is_some() {
            critical.data = "0".into();
        }
        self.lost_criticals.remove(&location);
        self.component_failures
            .retain(|failure| failure.location != location);
        if let Some(index) = weapon {
            self.weapon_failures.remove(&index);
            self.jammed_weapons.remove(&index);
            self.spent_launchers.remove(&index);
            self.powered_down_weapons.remove(&index);
            self.weapon_recycle.remove(&index);
        }
        if let Some(index) = ammunition {
            self.ammunition[index] = 0;
        }
        self.contract_loadout = true;
        self.live_mass.invalidate();
        Ok(())
    }
    /// Construct intact material state only when the template has complete equipment and mass rules.
    /// This does not certify construction legality or add a vehicle to the running world.
    pub fn new(definition: VehicleTemplate) -> Result<Self> {
        Self::new_mode(definition, false)
    }

    pub(crate) fn new_contract(definition: VehicleTemplate) -> Result<Self> {
        Self::new_mode(definition, true)
    }

    fn new_mode(definition: VehicleTemplate, contract_loadout: bool) -> Result<Self> {
        // The (tons + 5, at least 10) / 10 internal structure
        // (vehicle_int_check) is forced while the template file is read;
        // construction and saved-definition restore keep the stored internals
        // verbatim.
        definition.validate_anatomy()?;
        super::radio::validate_attributes(&definition.attributes)?;
        if !contract_loadout {
            definition.mass()?;
        }
        let loadout = if contract_loadout {
            VehicleLoadout::resolve_contract(&definition)?
        } else {
            VehicleLoadout::resolve(&definition)?
        };
        let vtol_fuel = definition
            .is_vtol()
            .then(|| super::VtolFuel::from_template(&definition))
            .transpose()?;
        let sections = Self::pristine_sections(&definition);
        let ammunition = loadout
            .ammunition
            .iter()
            .map(|bin| {
                if definition.sections[&bin.location.section].internal == 0 {
                    0
                } else {
                    bin.rounds
                }
            })
            .collect();
        let spent_launchers = loadout
            .weapons
            .iter()
            .enumerate()
            .filter_map(|(index, mount)| mount.initially_spent.then_some(index))
            .collect();
        let fire_modes = loadout
            .weapons
            .iter()
            .enumerate()
            .filter_map(|(index, mount)| {
                (mount.initial_fire_mode != super::FireMode::Normal)
                    .then_some((index, mount.initial_fire_mode))
            })
            .collect();
        let ammunition_modes = loadout
            .weapons
            .iter()
            .enumerate()
            .filter_map(|(index, mount)| {
                (mount.initial_ammunition_mode != super::AmmunitionMode::Normal)
                    .then_some((index, mount.initial_ammunition_mode))
            })
            .collect();
        let vtol_flight = definition.is_vtol().then(super::VtolFlight::default);
        Ok(Self {
            no_armor_warning: false,
            no_ammunition_warning: false,
            searchlight: Default::default(),
            autocon_shutdown: false,
            searchlight_warning: false,
            illumination_observed: false,
            auxiliary_preferences: Default::default(),
            base_movement_fields: Default::default(),
            shot_counters: Default::default(),
            damage_counters: Default::default(),
            units_killed: 0,
            last_startup: 0,
            display_name: Default::default(),
            markings: Default::default(),
            sixth_sense: Default::default(),
            cockpit_links: Default::default(),
            hardware: Default::default(),
            contract_loadout,
            administrative_raw: None,
            definition,
            sections,
            ammunition,
            position: None,
            detached: false,
            map_slot: None,
            battlefield_label: None,
            preferred_id: None,
            pilot: None,
            power: super::Power::Off,
            motion: None,
            detached_heading: 0.0,
            under_bridge: false,
            ground_elevation: None,
            free_fall: None,
            orbital_drop: None,
            propulsion: Default::default(),
            live_mass: Default::default(),
            critical_conditions: Default::default(),
            motive_speed_loss: 0.0,
            immobilized: false,
            turret_offset: 0.0,
            automatic_turret: false,
            turret_locked: false,
            turret_jammed: false,
            turret_repairs: Vec::new(),
            crew_stun_remaining: 0,
            crew_stun_condition: None,
            pilot_injuries: 0,
            pilot_killed: false,
            character_pilot: None,
            flooded: false,
            breached_sections: Default::default(),
            transport_destroyed: false,
            tail_rotor_destroyed: false,
            vtol_fuel,
            vtol_flight,
            crew_killed: false,
            hide_elapsed: None,
            self_destruct: None,
            self_destruct_safe: false,
            crew_recovery: super::Recovery::fresh(),
            piloting_damage: 0,
            gunnery_damage: 0,
            lost_stabilizers: std::collections::BTreeSet::new(),
            lost_criticals: std::collections::BTreeSet::new(),
            brief: Default::default(),
            tics: Default::default(),
            signature: Default::default(),
            scanner_perception: super::scanner::default_perception(),
            radio: Default::default(),
            radio_skill: super::radio::default_skill(),
            fired_recently: false,
            radio_experience_remaining: 0,
            towable: false,
            fortified: false,
            observer: false,
            weapons_hold: false,
            combat_safe: false,
            visibility: super::Visibility::default(),
            dig: super::DigState::default(),
            experience: Default::default(),
            friendly_fire_safety: false,
            auto_fall: false,
            // Anti-missile defense is purely reactive, so installed systems start armed.
            ams_enabled: loadout.weapons.iter().any(|mount| mount.weapon.is_ams()),
            weapon_heat: 0.0,
            inferno_remaining: 0,
            burning_sections: BTreeMap::new(),
            extinguishing: None,
            building_entry: None,
            beacons: BTreeMap::new(),
            target_lock: None,
            aimed_section: None,
            artillery_adjustment: 0,
            c3_network: None,
            c3i_network: None,
            network_automation: Default::default(),
            electronics: Default::default(),
            spotter: None,
            spotter_events: Default::default(),
            tag: Default::default(),
            contacts: Default::default(),
            fire_modes,
            ammunition_modes,
            ammunition_sections: BTreeMap::new(),
            unjam: None,
            pod_removal: None,
            jammed_weapons: Default::default(),
            component_failures: Vec::new(),
            weapon_failures: BTreeMap::new(),
            powered_down_weapons: Default::default(),
            weapon_recycle: BTreeMap::new(),
            spent_launchers,
            dice: super::Dice::fresh(),
        })
    }

    /// Absolute facing of a surviving turret; its offset is attached to the hull.
    pub fn turret_heading(&self) -> Option<f64> {
        self.sections
            .get(&VehicleSection::Turret)
            .filter(|state| state.internal > 0)
            .map(|_| (self.heading() + self.turret_offset).rem_euclid(360.0))
    }

    /// Whether damage has locked the surviving turret to its current hull-relative facing.
    pub fn turret_locked(&self) -> bool {
        self.turret_locked
    }

    /// Edit operating conditions without changing heading, hardware or repair countdowns.
    pub(super) fn set_turret_conditions(&mut self, locked: bool, jammed: bool) -> Result<()> {
        ensure!(
            !(locked || jammed) || self.turret_heading().is_some(),
            "Vehicle has no turret"
        );
        self.turret_locked = locked;
        self.turret_jammed = jammed;
        Ok(())
    }

    /// Lock a surviving turret; the damage adapter owns notification and transaction publication.
    pub fn lock_turret(&mut self) -> Result<()> {
        ensure!(self.turret_heading().is_some(), "Vehicle has no turret");
        self.turret_locked = true;
        self.turret_jammed = false;
        Ok(())
    }

    /// Disable propulsion, retaining the distinct standard and advanced immobility flags.
    pub(super) fn disable_engine(&mut self, advanced: bool) {
        self.motive_speed_loss = self.definition.max_speed;
        self.immobilized |= advanced;
        self.halt();
    }

    /// Stored weapon heat, including coolant credit; never sampled as ground-vehicle overheating.
    pub fn weapon_heat(&self) -> f64 {
        self.weapon_heat
    }

    /// Saved virtual-crew recovery, independent of mechanical crew stun.
    pub fn crew_recovery(&self) -> &super::Recovery {
        &self.crew_recovery
    }

    /// Current cockpit injury count, synchronized with character-mode injury reports.
    pub fn pilot_injuries(&self) -> u8 {
        self.pilot_injuries
    }

    /// Whether an event instantly killed the crew rather than accumulating pilot injuries.
    pub fn crew_killed(&self) -> bool {
        self.crew_killed
    }

    /// Record instant crew loss and use the same wreck cleanup as other vehicle destruction.
    pub(super) fn kill_crew(&mut self) {
        self.crew_killed = true;
        self.finish_destruction();
    }

    /// Reconcile tactical or character crew death without changing armor or equipment.
    pub(super) fn reconcile_crew_loss(&mut self) {
        if !self.pilot_killed && !self.character_pilot.is_some_and(|status| status.killed) {
            return;
        }
        self.finish_destruction();
    }

    /// Cancel active controls once when a vehicle becomes a wreck, preserving material state.
    pub(super) fn finish_destruction(&mut self) {
        super::spotter_events::clear(&mut self.spotter_events);
        self.self_destruct = None;
        self.hide_elapsed = None;
        self.cancel_digging();
        self.lose_vtol_lift();
        self.pilot = None;
        self.crew_recovery.clear();
        self.clear_fires();
        self.target_lock = None;
        self.power = super::Power::Off;
        self.searchlight.shutdown();
        self.crew_stun_remaining = 0;
        self.crew_stun_condition = None;
        self.turret_repairs.clear();
        self.halt();
        self.reconcile_electronics();
    }

    /// Disable transported machinery without inventing a hull hit or killing its crew.
    pub(super) fn destroy_with_transport(&mut self) {
        self.transport_destroyed = true;
        self.finish_destruction();
    }

    /// Whether water has permanently disabled this vehicle, regardless of surviving armor.
    pub fn flooded(&self) -> bool {
        self.flooded
    }

    /// Record water destruction after an environmental rule has established flooding.
    /// The caller owns immersion admission, notifications and any preceding fall damage.
    /// Returns false for an existing wreck without cancelling later wreck effects again.
    pub fn destroy_by_flooding(&mut self) -> bool {
        if self.is_destroyed() {
            return false;
        }
        self.flooded = true;
        self.finish_destruction();
        true
    }

    /// Current motive damage, expressed as lost maximum speed in kph.
    pub fn motive_speed_loss(&self) -> f64 {
        self.motive_speed_loss
    }

    /// Whether a motive-system hit has stopped all ground motion.
    pub fn immobilized(&self) -> bool {
        self.immobilized
    }

    /// Available throttle after motive damage, independently of the owned construction definition.
    pub fn maximum_speed(&self) -> f64 {
        if self.immobilized
            || self.rotor_destroyed()
            || self.is_destroyed()
            || self.definition.movement == super::VehicleMovement::Stationary
        {
            return 0.0;
        }
        self.propulsion
            .maximum((self.definition.max_speed - self.motive_speed_loss).max(0.0))
    }

    /// Apply an explicit motive consequence; damage adapters own notices and the enclosing transaction.
    pub fn apply_motive_hit(&mut self, hit: super::VehicleMotiveHit) {
        if self.immobilized
            || matches!(
                hit,
                super::VehicleMotiveHit::SpeedLoss { movement_points: 0 }
            )
        {
            return;
        }
        match hit {
            super::VehicleMotiveHit::Immobilize => self.immobilized = true,
            super::VehicleMotiveHit::SpeedLoss { movement_points } => {
                self.propulsion.lower(f64::from(movement_points) * 10.75);
                self.motive_speed_loss = (self.motive_speed_loss
                    + f64::from(movement_points) * 10.75)
                    .min(self.definition.max_speed);
            }
        }
        let maximum = self.maximum_speed();
        if maximum == 0.0 {
            self.halt();
            return;
        }
        if let Some(motion) = &mut self.motion {
            let direction = if motion.desired_speed < -0.1 {
                -1.0
            } else {
                1.0
            };
            let limit = maximum * if direction < 0.0 { 2.0 / 3.0 } else { 1.0 };
            if motion.desired_speed.abs() > limit {
                motion.desired_speed = limit * direction;
            }
            if motion.speed.abs() > limit {
                motion.speed = limit * direction;
            }
        }
    }

    /// Saved RPG injury status; character health determines death independently of tactical hits.
    pub fn character_pilot_status(&self) -> Option<super::CharacterPilotStatus> {
        self.character_pilot
    }

    /// Assigned cockpit operator, checked against world containment.
    pub fn pilot(&self) -> Option<crate::ObjectId> {
        self.pilot
    }

    /// Unix time supplied at the most recent completed startup, initially zero.
    pub fn last_startup(&self) -> i64 {
        self.last_startup
    }

    /// Saved engine lifecycle driven by committed one-second updates.
    pub fn power(&self) -> super::Power {
        self.power
    }

    /// Hovercraft passage below a bridge deck, retained independently of the map tile.
    pub fn under_bridge(&self) -> bool {
        self.under_bridge
    }

    /// Ground descent owns altitude and excludes powered translation and aircraft state.
    pub(super) fn validate_ground_descent(&self) -> Result<()> {
        ensure!(
            self.free_fall.is_none_or(|fall| !self.definition.is_vtol()
                && self.position.is_some()
                && self.ground_elevation.is_none()
                && !fall.grounded()
                && self
                    .motion
                    .is_some_and(|motion| motion.speed == 0.0 && motion.desired_speed == 0.0)),
            "Invalid ground vehicle descent state"
        );
        Ok(())
    }

    /// Current forced-descent cursor for either a ground chassis or rotorcraft.
    pub fn free_fall(&self) -> Option<super::FreeFall> {
        self.free_fall
            .or_else(|| self.vtol_flight.and_then(|flight| flight.fall))
    }

    /// Ground support height; hovercraft float at water level rather than on the bed.
    pub fn elevation_level(&self, tile: super::Hex) -> i32 {
        if let Some(drop) = self.orbital_drop {
            return drop.elevation();
        }
        if let Some(flight) = self.vtol_flight {
            return flight.altitude as i32;
        }
        if let Some(fall) = self.free_fall {
            return fall.elevation();
        }
        if let Some(level) = self.ground_elevation {
            return level as i32;
        }
        self.terrain_elevation(tile, self.under_bridge)
    }

    /// Support height at a destination, independent of retained altitude on the current hex.
    pub(super) fn terrain_elevation(&self, tile: super::Hex, under_bridge: bool) -> i32 {
        if under_bridge && tile.has_bridge() {
            return i32::from(tile.water_line());
        }
        if self.definition.movement == super::VehicleMovement::Hover && tile.is_water_surface() {
            return i32::from(tile.water_line());
        }
        i32::from(tile.standing_height())
    }

    /// Current continuous position and commanded motion.
    pub fn motion(&self) -> Option<super::Motion> {
        self.motion
    }

    /// Current facing, including the persisted off-map native heading.
    pub fn heading(&self) -> f64 {
        self.motion
            .map_or(self.detached_heading, |motion| motion.heading)
    }

    /// Commit a traced movement segment without resetting battlefield membership.
    pub(super) fn update_motion(
        &mut self,
        motion: super::Motion,
        position: super::Position,
        under_bridge: bool,
    ) {
        if self.position != Some(position) {
            self.ground_elevation = None;
        }
        self.under_bridge = under_bridge;
        self.detached_heading = motion.heading;
        self.motion = Some(motion);
        self.position = Some(position);
    }

    /// Restore a rejected terrain entry while preserving any fall-induced heading and damage.
    pub(super) fn restore_ground_position(
        &mut self,
        position: super::Position,
        point: super::Point,
        height: i16,
        under_bridge: bool,
    ) {
        let mut motion = self.motion.expect("Placed vehicle has motion");
        motion.point = point;
        self.update_motion(motion, position, under_bridge);
        self.ground_elevation = Some(f64::from(height));
    }

    /// Stop translation and turning while preserving the current point and facing.
    pub(super) fn halt(&mut self) {
        if let Some(motion) = &mut self.motion {
            motion.stop_translation();
            motion.desired_heading = motion.heading;
        }
    }

    /// Saved battlefield coordinates, absent when held outside a battlefield.
    pub fn position(&self) -> Option<super::Position> {
        (!self.detached).then_some(self.position).flatten()
    }

    /// Stable membership slot shared with other battlefield unit classes.
    pub fn map_slot(&self) -> Option<u32> {
        self.map_slot
    }

    /// Placement adapters update coordinates and slot together.
    pub(super) fn set_placement(&mut self, placement: Option<(super::Position, u32)>) {
        if self.position.map(|p| p.map) != placement.map(|(p, _)| p.map) {
            self.c3_network = None;
            self.tag.target = None;
            self.c3i_network = None;
        }
        self.under_bridge = false;
        self.ground_elevation = None;
        self.free_fall = None;
        self.orbital_drop = None;
        self.dig = super::DigState::default();
        if let Some(flight) = &mut self.vtol_flight {
            *flight = super::VtolFlight::default();
        }
        self.motion = placement.map(|(position, _)| {
            let mut motion = super::Motion::stationary(
                super::HexCoordinate {
                    x: i32::from(position.x),
                    y: i32::from(position.y),
                }
                .center(),
            );
            motion.heading = self.detached_heading;
            motion.desired_heading = self.detached_heading;
            motion
        });
        self.detached = false;
        self.battlefield_label = None;
        self.position = placement.map(|(position, _)| position);
        self.map_slot = placement.map(|(_, slot)| slot);
    }

    /// Last physical coordinates remain available after removal from the tactical map.
    pub(super) fn retained_position(&self) -> Option<super::Position> {
        self.position
    }

    /// Remove map membership without discarding controls or the saved physical pose.
    pub(super) fn detach_scenario_membership(&mut self) {
        self.detached = self.position.is_some();
        self.map_slot = None;
        self.c3_network = None;
        self.tag.target = None;
        self.c3i_network = None;
        self.dig = super::DigState::default();
        self.building_entry = None;
    }

    /// Replace tactical membership without resetting movement, altitude, flight or crew.
    pub(super) fn assign_membership(&mut self, position: super::Position, slot: u32) {
        self.detached = false;
        self.position = Some(position);
        self.map_slot = Some(slot);
        self.battlefield_label = None;
        self.c3_network = None;
        self.tag.target = None;
        self.c3i_network = None;
    }

    /// Immutable construction facts used for replay and equipment resolution.
    pub fn definition(&self) -> &VehicleTemplate {
        &self.definition
    }

    /// Construction baseline used by the shared attacker movement calculation.
    pub fn template_speed(&self) -> f64 {
        super::read_template_speed(&self.definition.attributes, self.definition.max_speed)
            .expect("validated template speed")
    }

    /// Set the independent firing-movement baseline without changing propulsion.
    pub(super) fn set_template_speed(&mut self, speed: f64) {
        super::write_template_speed(&mut self.definition.attributes, speed);
    }

    /// Change only the authored engine allocation override; recalculation is a separate operation.
    pub(super) fn set_engine_sink_override(&mut self, value: i32) {
        super::write_engine_sink_override(&mut self.definition.attributes, value);
    }

    /// Keep the owned tank definition and live baseline capacity consistent, retaining fuel.
    pub(super) fn set_original_fuel_capacity(&mut self, capacity: u32) -> Result<()> {
        self.vtol_fuel
            .as_mut()
            .context("Fuel capacity requires a VTOL")?
            .set_capacity(capacity);
        self.definition
            .attributes
            .insert("fuel".into(), capacity.to_string());
        Ok(())
    }

    /// Keep resolved locomotion and its authored attribute in agreement.
    pub(super) fn set_movement(&mut self, movement: super::VehicleMovement) {
        let name = match movement {
            super::VehicleMovement::Tracked => "Track",
            super::VehicleMovement::Wheeled => "Wheel",
            super::VehicleMovement::Hover => "Hover",
            super::VehicleMovement::Stationary => "None",
            super::VehicleMovement::Vtol => "VTOL",
        };
        self.definition.movement = movement;
        self.definition
            .attributes
            .insert("move_type".into(), name.into());
    }

    /// Change nominal tonnage without rebuilding equipment, protection or crew state.
    pub(super) fn set_nominal_tonnage(&mut self, tons: u16) {
        self.definition.tons = tons;
        self.definition
            .attributes
            .insert("tons".into(), tons.to_string());
    }

    /// Update the owned cargo installation; the shared load service reconciles motion.
    pub(super) fn set_cargo_space(&mut self, value: u32) {
        self.definition
            .attributes
            .insert("cargo_space".into(), value.to_string());
    }

    /// Edit identity through shared validation without rebuilding or resetting unit state.
    pub(super) fn set_identity(
        &mut self,
        field: super::unit_identity::IdentityField,
        value: &str,
    ) -> Result<()> {
        field.apply(
            &mut self.definition.name,
            &mut self.definition.reference,
            &mut self.definition.attributes,
            value,
        )
    }

    /// Current protection keyed by vehicle faces rather than Mech sections.
    pub fn sections(&self) -> &BTreeMap<VehicleSection, SectionState> {
        &self.sections
    }

    /// Current rounds in the stable order of the resolved loadout.
    pub fn ammunition(&self) -> &[u16] {
        &self.ammunition
    }

    /// Resolve derived equipment without storing a second copy in the snapshot.
    pub fn loadout(&self) -> Result<VehicleLoadout> {
        if let Some(projection) = super::loadout_context::vehicle(self) {
            return Ok(projection);
        }
        let projection =
            super::equipment_context::vehicle(&self.definition, self.contract_loadout)?;
        super::loadout_context::remember_vehicle(self, &projection);
        Ok(projection)
    }

    /// Flooding, crew loss or any lost hull face destroys a vehicle; turret loss alone does not.
    pub fn is_destroyed(&self) -> bool {
        self.transport_destroyed
            || self.flooded
            || self.crew_killed
            || self.pilot_killed
            || self.character_pilot.is_some_and(|status| status.killed)
            || self.hull_destroyed()
    }

    /// Hull loss is distinct from crew death, turret loss and rotor loss.
    pub(super) fn hull_destroyed(&self) -> bool {
        self.sections.iter().any(|(&section, state)| {
            !matches!(section, VehicleSection::Turret | VehicleSection::Rotor)
                && state.internal == 0
        })
    }

    /// Remove rounds after the calling attack has selected and validated its ammunition feed.
    /// Invalid bins, insufficient ammunition, and destroyed hulls leave the state unchanged.
    pub fn expend_ammunition(&mut self, bin: usize, rounds: u16) -> Result<()> {
        ensure!(
            !self.is_destroyed(),
            "Destroyed vehicle cannot expend ammunition"
        );
        self.expend_reserved_ammunition(bin, rounds)
    }

    /// Consume a validated firing reservation after shooter damage, including fatal misloads.
    /// The enclosing attack must clamp the draw to inventory surviving its damage cascade.
    pub(super) fn expend_reserved_ammunition(&mut self, bin: usize, rounds: u16) -> Result<()> {
        let remaining = self
            .ammunition
            .get_mut(bin)
            .context("Vehicle ammunition bin not found")?;
        ensure!(*remaining >= rounds, "Insufficient vehicle ammunition");
        *remaining -= rounds;
        if rounds > 0 {
            self.live_mass.invalidate();
        }
        Ok(())
    }

    /// Apply the common surviving-inventory rule after a misload has resolved vehicle damage.
    pub(super) fn spend_surviving_draws(
        &mut self,
        draws: Vec<super::AmmunitionDraw>,
    ) -> Vec<super::AmmunitionDraw> {
        let spent = super::ammunition_feed::spend_surviving_draws(&mut self.ammunition, draws);
        if !spent.is_empty() {
            self.live_mass.invalidate();
        }
        spent
    }

    /// Apply one protection phase, returning overflow for the combat caller to handle.
    /// Vehicle armor uses the selected hull face; the Mech rear-armor selector has no effect.
    /// This primitive does not transfer damage or roll criticals, and turret loss preserves the hull.
    pub fn damage_phase(
        &mut self,
        section: VehicleSection,
        amount: u16,
        phase: DamagePhase,
    ) -> Result<DamageResult<VehicleSection>> {
        ensure!(
            self.sections.contains_key(&section),
            "Vehicle section {} is absent",
            section.name()
        );
        let mut result = DamageResult {
            section,
            absorbed: 0,
            remaining: amount,
            destroyed_sections: Vec::new(),
            unit_destroyed: self.is_destroyed(),
        };
        if amount == 0 || self.sections[&section].internal == 0 {
            return Ok(result);
        }
        // Resolve before mutation so even a future equipment validation error is atomic.
        let loadout = self.loadout()?;
        let state = self
            .sections
            .get_mut(&section)
            .expect("validated vehicle face");
        let protection = match phase {
            DamagePhase::Armor { .. } => &mut state.armor,
            DamagePhase::Internal => &mut state.internal,
        };
        result.absorbed = (*protection).min(amount);
        *protection -= result.absorbed;
        if result.absorbed > 0 {
            self.live_mass.invalidate();
        }
        result.remaining -= result.absorbed;
        if matches!(phase, DamagePhase::Internal) && state.internal == 0 {
            state.armor = 0;
            state.rear = 0;
            for (index, bin) in loadout.ammunition.iter().enumerate() {
                if bin.location.section == section {
                    self.ammunition[index] = 0;
                }
            }
            if section == VehicleSection::Turret {
                self.turret_locked = false;
                self.turret_jammed = false;
                self.turret_repairs.clear();
                self.turret_offset = 0.0;
            }
            self.jammed_weapons
                .retain(|index| loadout.weapons[*index].criticals[0].section != section);
            self.weapon_failures
                .retain(|index, _| loadout.weapons[*index].criticals[0].section != section);
            self.weapon_recycle
                .retain(|index, _| loadout.weapons[*index].criticals[0].section != section);
            self.beacons.remove(&section);
            if section == VehicleSection::Rotor {
                self.lose_vtol_lift();
                self.halt();
            }
            result.destroyed_sections.push(section);
        }
        let was_destroyed = result.unit_destroyed;
        result.unit_destroyed = self.is_destroyed();
        // Destruction cancels active events once; subsequent damage can follow newly lit fires.
        if result.unit_destroyed && !was_destroyed {
            self.finish_destruction();
        } else {
            self.reconcile_electronics();
        }
        Ok(result)
    }
}

impl TryFrom<VehicleRecord> for Vehicle {
    type Error = anyhow::Error;

    /// Reject snapshots whose protection or ammunition cannot belong to the supplied construction.
    fn try_from(record: VehicleRecord) -> Result<Self> {
        Self::from_record(record).restored()
    }
}

impl Vehicle {
    /// Take every field from a saved record without checking it.
    fn from_record(record: VehicleRecord) -> Self {
        Self {
            contract_loadout: record.contract_loadout,
            administrative_raw: record.administrative_raw,
            auxiliary_preferences: record.auxiliary_preferences,
            base_movement_fields: record.base_movement_fields,
            shot_counters: record.shot_counters,
            damage_counters: record.damage_counters,
            units_killed: record.units_killed,
            no_armor_warning: record.no_armor_warning,
            no_ammunition_warning: record.no_ammunition_warning,
            searchlight: record.searchlight,
            autocon_shutdown: record.autocon_shutdown,
            searchlight_warning: record.searchlight_warning,
            illumination_observed: record.illumination_observed,
            display_name: record.display_name,
            markings: record.markings,
            sixth_sense: record.sixth_sense,
            last_startup: record.last_startup,
            cockpit_links: record.cockpit_links,
            hardware: record.hardware,
            self_destruct_safe: record.self_destruct_safe,
            self_destruct: record.self_destruct,
            hide_elapsed: record.hide_elapsed,
            towable: record.towable,
            fortified: record.fortified,
            observer: record.observer,
            weapons_hold: record.weapons_hold,
            combat_safe: record.combat_safe,
            visibility: record.visibility,
            dig: record.dig,
            definition: record.definition,
            sections: record.sections,
            ammunition: record.ammunition,
            position: record.position,
            detached: record.detached,
            map_slot: record.map_slot,
            battlefield_label: record.battlefield_label,
            preferred_id: record.preferred_id,
            pilot: record.pilot,
            power: record.power,
            motion: record.motion,
            detached_heading: record.detached_heading,
            under_bridge: record.under_bridge,
            ground_elevation: record.ground_elevation,
            free_fall: record.free_fall,
            orbital_drop: record.orbital_drop,
            propulsion: record.propulsion,
            live_mass: record.live_mass,
            critical_conditions: record.critical_conditions,
            motive_speed_loss: record.motive_speed_loss,
            immobilized: record.immobilized,
            turret_offset: record.turret_offset,
            automatic_turret: record.automatic_turret,
            turret_locked: record.turret_locked,
            turret_jammed: record.turret_jammed,
            turret_repairs: record.turret_repairs,
            crew_stun_remaining: record.crew_stun_remaining,
            crew_stun_condition: record.crew_stun_condition,
            pilot_injuries: record.pilot_injuries,
            pilot_killed: record.pilot_killed,
            character_pilot: record.character_pilot,
            flooded: record.flooded,
            breached_sections: record.breached_sections,
            transport_destroyed: record.transport_destroyed,
            tail_rotor_destroyed: record.tail_rotor_destroyed,
            vtol_fuel: record.vtol_fuel,
            vtol_flight: record.vtol_flight,
            crew_killed: record.crew_killed,
            crew_recovery: record.crew_recovery,
            piloting_damage: record.piloting_damage,
            gunnery_damage: record.gunnery_damage,
            lost_stabilizers: record.lost_stabilizers,
            lost_criticals: record.lost_criticals,
            brief: record.brief,
            tics: record.tics,
            signature: record.signature,
            scanner_perception: record.scanner_perception,
            radio: record.radio,
            radio_skill: record.radio_skill,
            fired_recently: record.fired_recently,
            radio_experience_remaining: record.radio_experience_remaining,
            experience: record.experience,
            friendly_fire_safety: record.friendly_fire_safety,
            auto_fall: record.auto_fall,
            ams_enabled: record.ams_enabled,
            weapon_heat: record.weapon_heat,
            inferno_remaining: record.inferno_remaining,
            burning_sections: record.burning_sections,
            extinguishing: record.extinguishing,
            building_entry: record.building_entry,
            beacons: record.beacons,
            target_lock: record.target_lock,
            aimed_section: record.aimed_section,
            artillery_adjustment: record.artillery_adjustment,
            c3_network: record.c3_network,
            c3i_network: record.c3i_network,
            network_automation: record.network_automation,
            electronics: record.electronics,
            spotter: record.spotter,
            spotter_events: record.spotter_events,
            tag: record.tag,
            contacts: record.contacts,
            fire_modes: record.fire_modes,
            ammunition_modes: record.ammunition_modes,
            ammunition_sections: record.ammunition_sections,
            unjam: record.unjam,
            pod_removal: record.pod_removal,
            jammed_weapons: record.jammed_weapons,
            component_failures: record.component_failures,
            weapon_failures: record.weapon_failures,
            weapon_recycle: record.weapon_recycle,
            powered_down_weapons: record.powered_down_weapons,
            spent_launchers: record.spent_launchers,
            dice: record.dice,
        }
    }

    /// Accept a vehicle taken from a saved record or a fixture edit: validate it as
    /// given, then leave any command network its computers can no longer hold.
    pub(super) fn restored(mut self) -> Result<Self> {
        self.validate()?;
        self.settle_command_networks();
        Ok(self)
    }

    /// Section protection as built, before any damage.
    fn pristine_sections(definition: &VehicleTemplate) -> BTreeMap<VehicleSection, SectionState> {
        definition
            .sections
            .iter()
            .map(|(&section, layout)| {
                let state = if layout.internal == 0 {
                    SectionState {
                        armor: 0,
                        internal: 0,
                        rear: 0,
                    }
                } else {
                    SectionState {
                        armor: layout.armor,
                        internal: layout.internal,
                        rear: layout.rear,
                    }
                };
                (section, state)
            })
            .collect()
    }

    /// Reject a vehicle whose state cannot belong to its construction template.
    ///
    /// Loading a saved record, fixture edits and the world's own checks all use this, so a
    /// vehicle that passes here is one a restart would accept.
    pub(crate) fn validate(&self) -> Result<()> {
        let _measurement = super::autopilot::diagnostics::combat("validation_unit");
        self.definition.validate_anatomy()?;
        super::radio::validate_attributes(&self.definition.attributes)?;
        if !self.contract_loadout {
            self.definition.mass()?;
        }
        self.loadout()?;
        if self.definition.is_vtol() {
            super::VtolFuel::from_template(&self.definition)?;
        }
        let pristine = Self::pristine_sections(&self.definition);
        ensure!(
            self.vtol_fuel.is_some() == self.definition.is_vtol(),
            "Fuel state does not match vehicle class"
        );
        if let Some(fuel) = self.vtol_fuel {
            fuel.validate(&self.definition)?;
        }
        ensure!(
            self.vtol_flight.is_some() == self.definition.is_vtol(),
            "Flight state does not match vehicle class"
        );
        if let Some(flight) = self.vtol_flight {
            flight.validate()?;
        }
        ensure!(
            self.pilot_injuries <= i8::MAX as u8,
            "Invalid vehicle pilot injury count"
        );
        if let Some(timer) = self.self_destruct {
            timer.validate()?;
        }
        ensure!(
            self.hide_elapsed.is_none_or(|elapsed| elapsed <= 100),
            "Invalid hiding timer"
        );
        ensure!(
            (!self.crew_killed
                && !self.pilot_killed
                && !self.character_pilot.is_some_and(|status| status.killed))
                || (self.pilot.is_none()
                    && self.crew_stun_remaining == 0
                    && self.crew_stun_condition != Some(true)),
            "Dead vehicle crew retains pilot or stun"
        );
        ensure!(
            self.sections.keys().eq(pristine.keys()),
            "Vehicle snapshot sections do not match definition"
        );
        for (&section, state) in &self.sections {
            let original = &pristine[&section];
            ensure!(
                self.contract_loadout
                    || (state.armor <= original.armor
                        && state.internal <= original.internal
                        && state.rear <= original.rear),
                "Vehicle snapshot protection exceeds definition in {}",
                section.name()
            );
            ensure!(
                self.contract_loadout
                    || state.internal > 0
                    || (state.armor == 0 && state.rear == 0),
                "Destroyed vehicle section retains armor"
            );
        }
        ensure!(
            self.lost_criticals.iter().all(|location| self
                .definition
                .sections
                .get(&location.section)
                .is_some_and(|section| section.criticals.contains_key(&location.slot))),
            "Invalid destroyed vehicle equipment slot"
        );
        ensure!(
            self.piloting_damage <= 127 && self.gunnery_damage <= 127,
            "Invalid vehicle control penalty"
        );
        ensure!(
            self.lost_stabilizers
                .iter()
                .all(|section| self.sections.contains_key(section)),
            "Invalid vehicle stabilizer section"
        );
        ensure!(
            self.crew_stun_remaining <= 60
                && (!self
                    .sections
                    .iter()
                    .any(|(section, state)| *section != VehicleSection::Turret
                        && state.internal == 0)
                    || (self.crew_stun_remaining == 0 && self.crew_stun_condition != Some(true))),
            "Invalid vehicle crew stun countdown"
        );
        ensure!(
            self.breached_sections
                .iter()
                .all(|section| self.sections.contains_key(section)),
            "Invalid breached vehicle section"
        );
        let loadout = self.loadout()?;
        self.tics.validate(loadout.weapons.len())?;
        ensure!(
            self.ammunition.len() == loadout.ammunition.len(),
            "Vehicle snapshot ammunition bins do not match definition"
        );
        for (bin, &rounds) in loadout.ammunition.iter().zip(&self.ammunition) {
            ensure!(
                rounds <= bin.capacity,
                "Vehicle snapshot ammunition exceeds capacity"
            );
            ensure!(
                (self.sections[&bin.location.section].internal > 0
                    && !self.critical_destroyed(bin.location))
                    || rounds == 0,
                "Destroyed vehicle section retains ammunition"
            );
        }
        ensure!(
            (!self.detached && self.position.is_some()) == self.map_slot.is_some(),
            "Vehicle position and battlefield slot must agree"
        );
        ensure!(
            !self.detached || self.position.is_some(),
            "Detached vehicle lacks retained coordinates"
        );
        if let Some(position) = self.position {
            ensure!(
                position.map.0 >= 0 && position.x <= 999 && position.y <= 999,
                "Invalid vehicle battlefield coordinates"
            );
        }
        if let super::Power::Starting { remaining } = self.power {
            ensure!(
                (1..=30).contains(&remaining),
                "Invalid vehicle startup countdown"
            );
        }
        ensure!(
            self.power == super::Power::Off || self.position.is_some(),
            "Powered vehicle requires a battlefield"
        );
        ensure!(
            self.motion.is_some() == self.position.is_some(),
            "Vehicle motion and position must agree"
        );
        ensure!(
            self.motive_speed_loss.is_finite()
                && self.motive_speed_loss >= 0.0
                && self.motive_speed_loss <= self.definition.max_speed,
            "Invalid vehicle motive speed loss"
        );
        self.critical_conditions.validate(None)?;
        self.live_mass.validate()?;
        self.propulsion.validate()?;
        ensure!(
            !self.rotor_destroyed()
                || self
                    .motion
                    .map(|motion| motion.propelled(self.power))
                    .transpose()?
                    .is_none_or(|motion| !motion.active()),
            "Rotorless aircraft retains horizontal motion"
        );
        let maximum = super::speed_bonus::saved_limit(self.maximum_speed(), false, false, false);
        if let Some(motion) = self.motion {
            let propelled = motion.propelled(self.power)?;
            propelled.validate(maximum + 10.75)?;
            ensure!(
                self.maximum_speed() > 0.0 || !propelled.active(),
                "Immobile vehicle retains motion"
            );
            ensure!(
                motion.desired_speed >= -maximum * 2.0 / 3.0 && motion.desired_speed <= maximum,
                "Invalid vehicle throttle"
            );
            let position = self.position.expect("validated placement");
            let coordinate = motion.point.containing_hex()?;
            ensure!(
                coordinate.x == i32::from(position.x) && coordinate.y == i32::from(position.y),
                "Vehicle continuous position differs from hex"
            );
            ensure!(
                self.power == super::Power::Running
                    || !propelled.translating()
                    || super::vtol_flight::idle_controls(self.power, self.vtol_flight, self.motion),
                "Inactive vehicle retains motion"
            );
        }
        ensure!(
            !self.under_bridge
                || (self.position.is_some()
                    && self.definition.movement == super::VehicleMovement::Hover),
            "Invalid vehicle under-bridge state"
        );
        ensure!(
            self.ground_elevation
                .is_none_or(|height| self.position.is_some()
                    && height.is_finite()
                    && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&height)),
            "Retained elevation requires placement"
        );
        ensure!(
            self.detached_heading.is_finite() && (0.0..360.0).contains(&self.detached_heading),
            "Invalid retained vehicle heading"
        );
        ensure!(
            !self.tail_rotor_destroyed || self.definition.is_vtol(),
            "Ground vehicle retains tail rotor damage"
        );
        ensure!(
            self.turret_offset.is_finite() && (0.0..360.0).contains(&self.turret_offset),
            "Invalid vehicle turret facing"
        );
        ensure!(
            self.turret_heading().is_some() || (!self.turret_locked && self.turret_offset == 0.0),
            "Missing turret retains facing or lock"
        );
        ensure!(
            !self.turret_jammed || self.turret_heading().is_some(),
            "Invalid vehicle turret jam"
        );
        ensure!(
            self.turret_repairs.len() <= 256
                && self
                    .turret_repairs
                    .iter()
                    .all(|remaining| (1..=60).contains(remaining))
                && (self.turret_repairs.is_empty()
                    || (!self.is_destroyed() && self.turret_heading().is_some())),
            "Invalid vehicle turret repair countdown"
        );
        ensure!(
            !self.automatic_turret
                || self
                    .definition
                    .sections
                    .contains_key(&VehicleSection::Turret),
            "Automatic tracking without an authored turret"
        );

        ensure!(
            self.weapon_recycle.iter().all(|(index, remaining)| loadout
                .weapons
                .get(*index)
                .is_some_and(|mount| *remaining > 0
                    && *remaining
                        <= if self.weapon_failures.contains_key(index) {
                            120
                        } else {
                            super::weapon_settings::MAX_RECYCLE_SECONDS
                        }
                    && !self.critical_unavailable(mount.criticals[0]))),
            "Invalid vehicle weapon recycle timer"
        );
        ensure!(
            self.spent_launchers.iter().all(|index| loadout
                .weapons
                .get(*index)
                .is_some_and(|mount| self.contract_loadout || mount.one_shot)),
            "Invalid spent vehicle launcher"
        );
        ensure!(
            loadout
                .weapons
                .iter()
                .enumerate()
                .all(|(index, mount)| self.contract_loadout
                    || !mount.initially_spent
                    || self.spent_launchers.contains(&index)),
            "Initially spent vehicle launcher was reloaded"
        );
        ensure!(
            self.fire_modes
                .iter()
                .all(|(index, mode)| *mode != super::FireMode::Normal
                    && loadout
                        .weapons
                        .get(*index)
                        .is_some_and(|mount| self.contract_loadout || mode.supports(mount.weapon))),
            "Invalid vehicle firing mode"
        );
        ensure!(
            self.ammunition_modes.iter().all(|(index, mode)| *mode
                != super::AmmunitionMode::Normal
                && loadout
                    .weapons
                    .get(*index)
                    .is_some_and(|mount| self.contract_loadout || mode.supports(mount.weapon))),
            "Invalid vehicle ammunition selection"
        );
        super::component_failure::validate(&self.component_failures, |location| {
            loadout.systems.iter().any(|part| part.location == location)
                && !super::damage_field::placeholder(
                    &self.definition().sections[&location.section].criticals[&location.slot]
                        .equipment,
                )
        })?;
        super::weapon_failure::validate(&self.weapon_failures, loadout.weapons.len(), |index| {
            !self.critical_unavailable(loadout.weapons[index].criticals[0])
        })?;
        ensure!(
            self.jammed_weapons.iter().all(|index| loadout
                .weapons
                .get(*index)
                .is_some_and(|mount| mount.weapon.profile().ammunition_per_ton > 0
                    && !self.critical_unavailable(mount.criticals[0]))),
            "Invalid vehicle ammunition-feed jam"
        );
        ensure!(
            self.unjam
                .is_none_or(|attempt| (1..=60).contains(&attempt.remaining)
                    && loadout
                        .weapons
                        .get(attempt.weapon_index)
                        .is_some_and(|mount| mount.weapon.profile().ammunition_per_ton > 0)),
            "Invalid vehicle unjam attempt"
        );
        super::weapon_power::validate(&self.powered_down_weapons, &loadout.weapons)?;
        ensure!(
            self.target_lock
                .is_none_or(|lock| lock.remaining() <= 8 && self.power == super::Power::Running),
            "Invalid vehicle target lock countdown or power state"
        );
        if let Some(selection) = self.aimed_section {
            selection.validate()?;
        }
        ensure!(
            self.spotter.is_none_or(|id| id.0 > 0),
            "Invalid spotter identity"
        );
        self.spotter_events.validate()?;
        self.tag.validate()?;
        ensure!(
            self.c3_network != Some(0) && self.c3i_network != Some(0),
            "Invalid vehicle command-network identity"
        );
        ensure!(
            self.electronics_settled(),
            "Invalid vehicle electronic emission state"
        );
        self.brief.validate()?;
        super::radio::validate_channels(&self.radio)?;
        super::radio::validate_attributes(&self.definition().attributes)?;
        ensure!(
            self.radio_experience_remaining <= 61,
            "Invalid radio experience countdown"
        );
        self.hardware.validate()?;
        self.searchlight
            .validate(self.definition().has_special("Searchlight"))?;
        self.experience.validate()?;
        ensure!(
            self.beacons.iter().all(|(section, kinds)| !kinds.is_empty()
                && self
                    .sections
                    .get(section)
                    .is_some_and(|state| state.internal > 0)),
            "Vehicle beacon requires a surviving section and a nonempty effect set"
        );
        ensure!(
            self.pod_removal
                .is_none_or(|remaining| (1..=60).contains(&remaining)),
            "Invalid vehicle pod-removal countdown"
        );
        ensure!(self.weapon_heat.is_finite(), "Invalid vehicle weapon heat");
        ensure!(
            self.inferno_remaining <= i32::MAX as u32,
            "Invalid inferno duration"
        );
        ensure!(
            self.burning_sections
                .iter()
                .all(|(section, remaining)| self.sections.contains_key(section)
                    && (1..=60).contains(remaining)),
            "Invalid vehicle section fire"
        );
        ensure!(
            self.extinguishing
                .is_none_or(|remaining| (1..=120).contains(&remaining)),
            "Invalid vehicle extinguishing countdown"
        );
        if let Some(entry) = self.building_entry {
            entry.validate()?;
        }
        super::ammunition_preference::validate(
            &self.ammunition_sections,
            &loadout.weapons,
            |section| {
                self.definition
                    .sections
                    .get(&section)
                    .is_some_and(|s| s.internal > 0)
            },
        )?;
        ensure!(
            !self.is_destroyed() || self.power == super::Power::Off,
            "Destroyed vehicle must be shut down"
        );
        super::crew_recovery::validate(&self.crew_recovery, self.pilot(), self.pilot_injuries())?;
        self.validate_flight_state()?;
        self.validate_orbital_drop()?;
        self.validate_dig()?;
        Ok(())
    }
}

impl Vehicle {
    /// Common identity projection for object registration and saved-game inspection.
    pub(crate) fn identity(&self) -> super::StoredBattleUnit {
        super::StoredBattleUnit {
            name: self.definition.name.clone(),
            template: self.definition.reference.clone(),
            class_code: if self.definition.is_vtol() { 2 } else { 1 },
            movement_code: match self.definition.movement {
                super::VehicleMovement::Tracked => 1,
                super::VehicleMovement::Wheeled => 2,
                super::VehicleMovement::Hover => 3,
                super::VehicleMovement::Stationary => 10,
                super::VehicleMovement::Vtol => 4,
            },
            tons: i64::from(self.definition.tons),
            map: self.position().map(|position| position.map),
        }
    }
}

/// Attach owned vehicle state to an unused live thing; battlefield admission is separate.
pub fn create_vehicle(
    world: &mut crate::World,
    id: crate::ObjectId,
    definition: VehicleTemplate,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| object.kind == crate::Kind::Thing
                && !object.flags.contains(crate::Flag::Going)),
        "Vehicle target must be a live thing"
    );
    ensure!(
        !world.btech.registrations.contains_key(&id)
            && !world.btech.units.contains_key(&id)
            && !world.btech.maps.contains_key(&id),
        "Object already has BattleTech state"
    );
    super::inventory_mass(world, id)?;
    let vehicle = Vehicle::new(definition)?;
    world.btech.units.insert(id, vehicle.identity());
    world.btech.vehicles.insert(id, vehicle);
    std::sync::Arc::make_mut(&mut world.btech.registrations).insert(id, "MECH".into());
    Ok(())
}

/// Apply a vehicle material phase in a world candidate; the combat caller owns subsequent effects.
pub fn damage_vehicle_phase(
    world: &mut crate::World,
    id: crate::ObjectId,
    section: VehicleSection,
    amount: u16,
    phase: DamagePhase,
) -> Result<DamageResult<VehicleSection>> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Vehicle object is unavailable"
    );
    world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Constructed vehicle not found")?
        .damage_phase(section, amount, phase)
}

/// Apply vehicle motive damage inside the caller's world candidate; adapters own notifications.
pub fn damage_vehicle_motive(
    world: &mut crate::World,
    id: crate::ObjectId,
    hit: super::VehicleMotiveHit,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Vehicle object is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Constructed vehicle not found")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is already destroyed");
    vehicle.apply_motive_hit(hit);
    Ok(())
}
