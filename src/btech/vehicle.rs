//! Shared vehicle and rotorcraft material state with validated replay and explicit damage phases.
use super::{
    BattleDamagePhase, BattleDamageResult, BattleSectionState, BattleVehicleLoadout,
    BattleVehicleSection, BattleVehicleTemplate,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Vehicle protection and ammunition, independent of Mech anatomy and world admission.
/// Combat adapters must still resolve hit locations, criticals, crew effects and notifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "VehicleRecord")]
pub struct BattleVehicle {
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
    pub(super) tight_turn_mode: bool,
    pub(super) no_armor_warning: bool,
    pub(super) no_ammunition_warning: bool,
    pub(super) searchlight: super::BattleSearchlight,
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
    pub(super) self_destruct: Option<super::BattleSelfDestruct>,
    /// Durable temporary sensor-flash recovery.
    pub(super) blinded_remaining: u8,
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
    /// Saved sprint mode, independent of equipment boosts and requested speed.
    #[serde(default)]
    pub(super) sprinting: bool,
    /// Operator-imposed immunity to combat damage.
    #[serde(default)]
    pub(super) combat_safe: bool,
    /// Operator visibility privileges, independent of sensor equipment.
    #[serde(default)]
    pub(super) visibility: super::BattleVisibility,
    pub(super) dig: super::BattleDigState,
    definition: BattleVehicleTemplate,
    pub(super) sections: BTreeMap<BattleVehicleSection, BattleSectionState>,
    pub(super) ammunition: Vec<u16>,
    position: Option<super::BattlePosition>,
    /// Keep the physical pose after tactical membership is removed.
    #[serde(default)]
    pub(super) detached: bool,
    map_slot: Option<u32>,
    #[serde(default)]
    pub(super) battlefield_label: Option<String>,
    /// Optional configured identity, used only when selecting a new battlefield ID.
    #[serde(default)]
    pub(super) preferred_id: Option<super::BattlePreferredId>,
    pub(super) pilot: Option<crate::ObjectId>,
    pub(super) power: super::BattlePower,
    pub(super) motion: Option<super::BattleMotion>,
    pub(super) under_bridge: bool,
    pub(super) ground_elevation: Option<f64>,
    /// Ground chassis use the same forced-descent clock as aircraft after losing support.
    pub(super) free_fall: Option<super::BattleFreeFall>,
    /// Ground chassis share the Mech cocoon and jump-jet descent model.
    #[serde(default)]
    pub(super) orbital_drop: Option<super::BattleOrbitalDrop>,
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
    pub(super) character_pilot: Option<super::BattleCharacterPilotStatus>,
    /// Permanent water destruction, independent of hull integrity and crew injury.
    flooded: bool,
    /// Sections whose equipment was disabled by vacuum exposure.
    #[serde(default)]
    pub(super) breached_sections: std::collections::BTreeSet<BattleVehicleSection>,
    /// Hull disabled by destruction of its carrier, independently of crew injury.
    transport_destroyed: bool,
    /// Tail rotor loss constrains flight controls without destroying the hull.
    pub(super) tail_rotor_destroyed: bool,
    pub(super) vtol_fuel: Option<super::BattleVtolFuel>,
    pub(super) vtol_flight: Option<super::BattleVtolFlight>,
    /// Instant loss of the entire crew, independent of accumulated pilot injuries.
    crew_killed: bool,
    pub(super) crew_recovery: super::BattleRecovery,
    pub(super) piloting_damage: u8,
    pub(super) gunnery_damage: u8,
    pub(super) lost_stabilizers: std::collections::BTreeSet<BattleVehicleSection>,
    pub(super) lost_criticals: std::collections::BTreeSet<super::VehicleCriticalLocation>,
    pub(super) brief: super::BattleBriefSettings,
    pub(super) tics: super::BattleTics,
    pub(super) sensor_signature: super::BattleSensorSignature,
    pub(super) scanner_perception: i16,
    pub(super) radio: [super::BattleRadioChannel; 16],
    pub(super) radio_skill: i16,
    pub(super) sensor_signal: super::BattleSensorSignal,
    pub(super) fired_recently: bool,
    pub(super) radio_experience_remaining: u8,
    pub(super) experience: super::BattleUnitExperience,
    pub(super) friendly_fire_safety: bool,
    pub(super) auto_fall: bool,
    pub(super) ams_enabled: bool,
    /// Passive weapon heat and coolant credit; ground vehicles do not sample overheating.
    pub(super) weapon_heat: f64,
    pub(super) inferno_remaining: u32,
    pub(super) burning_sections: BTreeMap<BattleVehicleSection, u8>,
    pub(super) extinguishing: Option<u8>,
    pub(super) building_entry: Option<super::BattleBuildingEntry>,
    pub(super) beacons:
        BTreeMap<BattleVehicleSection, std::collections::BTreeSet<super::BattleBeaconKind>>,
    pub(super) target_lock: Option<super::BattleTargetSelection>,
    #[serde(default)]
    pub(super) aimed_section: Option<super::BattleAimSelection>,
    pub(super) artillery_adjustment: u8,
    pub(super) c3_network: Option<u64>,
    pub(super) c3i_network: Option<u64>,
    pub(super) electronics: super::BattleElectronics,
    pub(super) spotter: Option<crate::ObjectId>,
    pub(super) spotter_events: super::BattleSpotterEvents,
    pub(super) tag: super::BattleTagState,
    pub(super) contacts: BTreeMap<crate::ObjectId, super::BattleContact>,
    pub(super) sensor_selection: super::BattleSensorSelection,
    pub(super) fire_modes: BTreeMap<usize, super::BattleFireMode>,
    pub(super) ammunition_modes: BTreeMap<usize, super::BattleAmmunitionMode>,
    pub(super) ammunition_sections: BTreeMap<usize, BattleVehicleSection>,
    pub(super) unjam: Option<super::BattleUnjam>,
    pub(super) pod_removal: Option<u8>,
    pub(super) jammed_weapons: std::collections::BTreeSet<usize>,
    #[serde(default)]
    pub(super) component_failures:
        Vec<super::BattleComponentFailure<super::VehicleCriticalLocation>>,
    pub(super) weapon_failures: BTreeMap<usize, super::BattleEquipmentFailure>,
    pub(super) weapon_recycle: BTreeMap<usize, u16>,
    /// Gauss mounts deliberately powered down; independent of material damage.
    #[serde(default)]
    pub(super) powered_down_weapons: std::collections::BTreeSet<usize>,
    pub(super) spent_launchers: std::collections::BTreeSet<usize>,
    /// Replayable outcomes owned by this vehicle and committed with its effects.
    pub(super) dice: super::BattleDice,
}

/// Snapshot input is validated against its owned definition before becoming domain state.
#[derive(Deserialize)]
struct VehicleRecord {
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
    tight_turn_mode: bool,
    #[serde(default)]
    no_armor_warning: bool,
    #[serde(default)]
    no_ammunition_warning: bool,
    #[serde(default)]
    searchlight: super::BattleSearchlight,
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
    self_destruct: Option<super::BattleSelfDestruct>,
    #[serde(default)]
    blinded_remaining: u8,
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
    sprinting: bool,
    #[serde(default)]
    combat_safe: bool,
    #[serde(default)]
    visibility: super::BattleVisibility,
    #[serde(default)]
    dig: super::BattleDigState,
    definition: BattleVehicleTemplate,
    sections: BTreeMap<BattleVehicleSection, BattleSectionState>,
    ammunition: Vec<u16>,
    position: Option<super::BattlePosition>,
    /// Keep the physical pose after tactical membership is removed.
    #[serde(default)]
    pub(super) detached: bool,
    map_slot: Option<u32>,
    #[serde(default)]
    battlefield_label: Option<String>,
    #[serde(default)]
    preferred_id: Option<super::BattlePreferredId>,
    pilot: Option<crate::ObjectId>,
    power: super::BattlePower,
    motion: Option<super::BattleMotion>,
    under_bridge: bool,
    ground_elevation: Option<f64>,
    #[serde(default)]
    free_fall: Option<super::BattleFreeFall>,
    #[serde(default)]
    orbital_drop: Option<super::BattleOrbitalDrop>,
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
    turret_repairs: Vec<u8>,
    crew_stun_remaining: u8,
    #[serde(default)]
    crew_stun_condition: Option<bool>,
    pilot_injuries: u8,
    #[serde(default)]
    pilot_killed: bool,
    character_pilot: Option<super::BattleCharacterPilotStatus>,
    flooded: bool,
    #[serde(default)]
    breached_sections: std::collections::BTreeSet<BattleVehicleSection>,
    /// Hull disabled by destruction of its carrier, independently of crew injury.
    transport_destroyed: bool,
    /// Tail rotor loss constrains flight controls without destroying the hull.
    tail_rotor_destroyed: bool,
    vtol_fuel: Option<super::BattleVtolFuel>,
    vtol_flight: Option<super::BattleVtolFlight>,
    /// Instant loss of the entire crew, independent of accumulated pilot injuries.
    crew_killed: bool,
    crew_recovery: super::BattleRecovery,
    piloting_damage: u8,
    gunnery_damage: u8,
    lost_stabilizers: std::collections::BTreeSet<BattleVehicleSection>,
    lost_criticals: std::collections::BTreeSet<super::VehicleCriticalLocation>,
    brief: super::BattleBriefSettings,
    #[serde(default)]
    tics: super::BattleTics,
    sensor_signature: super::BattleSensorSignature,
    scanner_perception: i16,
    #[serde(default)]
    radio: [super::BattleRadioChannel; 16],
    #[serde(default = "super::radio::default_skill")]
    radio_skill: i16,
    #[serde(default)]
    sensor_signal: super::BattleSensorSignal,
    #[serde(default)]
    fired_recently: bool,
    #[serde(default)]
    radio_experience_remaining: u8,
    experience: super::BattleUnitExperience,
    friendly_fire_safety: bool,
    auto_fall: bool,
    ams_enabled: bool,
    weapon_heat: f64,
    inferno_remaining: u32,
    burning_sections: BTreeMap<BattleVehicleSection, u8>,
    extinguishing: Option<u8>,
    #[serde(default)]
    building_entry: Option<super::BattleBuildingEntry>,
    beacons: BTreeMap<BattleVehicleSection, std::collections::BTreeSet<super::BattleBeaconKind>>,
    target_lock: Option<super::BattleTargetSelection>,
    #[serde(default)]
    aimed_section: Option<super::BattleAimSelection>,
    artillery_adjustment: u8,
    c3_network: Option<u64>,
    c3i_network: Option<u64>,
    electronics: super::BattleElectronics,
    spotter: Option<crate::ObjectId>,
    #[serde(default)]
    spotter_events: super::BattleSpotterEvents,
    #[serde(default)]
    tag: super::BattleTagState,
    contacts: BTreeMap<crate::ObjectId, super::BattleContact>,
    sensor_selection: super::BattleSensorSelection,
    fire_modes: BTreeMap<usize, super::BattleFireMode>,
    ammunition_modes: BTreeMap<usize, super::BattleAmmunitionMode>,
    #[serde(default)]
    ammunition_sections: BTreeMap<usize, BattleVehicleSection>,
    unjam: Option<super::BattleUnjam>,
    pod_removal: Option<u8>,
    jammed_weapons: std::collections::BTreeSet<usize>,
    #[serde(default)]
    component_failures: Vec<super::BattleComponentFailure<super::VehicleCriticalLocation>>,
    weapon_failures: BTreeMap<usize, super::BattleEquipmentFailure>,
    weapon_recycle: BTreeMap<usize, u16>,
    #[serde(default)]
    powered_down_weapons: std::collections::BTreeSet<usize>,
    spent_launchers: std::collections::BTreeSet<usize>,
    dice: super::BattleDice,
}

impl BattleVehicle {
    /// Construct intact material state only when the template has complete equipment and mass rules.
    /// This does not certify construction legality or add a vehicle to the running world.
    pub fn new(definition: BattleVehicleTemplate) -> Result<Self> {
        definition.validate_anatomy()?;
        super::radio::validate_attributes(&definition.attributes)?;
        definition.mass()?;
        let loadout = BattleVehicleLoadout::resolve(&definition)?;
        let vtol_fuel = definition
            .is_vtol()
            .then(|| super::BattleVtolFuel::from_template(&definition))
            .transpose()?;
        let sections = definition
            .sections
            .iter()
            .map(|(&section, layout)| {
                let state = if layout.internal == 0 {
                    BattleSectionState {
                        armor: 0,
                        internal: 0,
                        rear: 0,
                    }
                } else {
                    BattleSectionState {
                        armor: layout.armor,
                        internal: layout.internal,
                        rear: layout.rear,
                    }
                };
                (section, state)
            })
            .collect();
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
                (mount.initial_fire_mode != super::BattleFireMode::Normal)
                    .then_some((index, mount.initial_fire_mode))
            })
            .collect();
        let ammunition_modes = loadout
            .weapons
            .iter()
            .enumerate()
            .filter_map(|(index, mount)| {
                (mount.initial_ammunition_mode != super::BattleAmmunitionMode::Normal)
                    .then_some((index, mount.initial_ammunition_mode))
            })
            .collect();
        let vtol_flight = definition.is_vtol().then(super::BattleVtolFlight::default);
        Ok(Self {
            tight_turn_mode: false,
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
            definition,
            sections,
            ammunition,
            position: None,
            detached: false,
            map_slot: None,
            battlefield_label: None,
            preferred_id: None,
            pilot: None,
            power: super::BattlePower::Off,
            motion: None,
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
            blinded_remaining: 0,
            hide_elapsed: None,
            self_destruct: None,
            self_destruct_safe: false,
            crew_recovery: super::BattleRecovery::fresh(),
            piloting_damage: 0,
            gunnery_damage: 0,
            lost_stabilizers: std::collections::BTreeSet::new(),
            lost_criticals: std::collections::BTreeSet::new(),
            brief: Default::default(),
            tics: Default::default(),
            sensor_signature: Default::default(),
            scanner_perception: super::scanner::default_perception(),
            radio: Default::default(),
            radio_skill: super::radio::default_skill(),
            sensor_signal: Default::default(),
            fired_recently: false,
            radio_experience_remaining: 0,
            towable: false,
            fortified: false,
            observer: false,
            weapons_hold: false,
            sprinting: false,
            combat_safe: false,
            visibility: super::BattleVisibility::default(),
            dig: super::BattleDigState::default(),
            experience: Default::default(),
            friendly_fire_safety: false,
            auto_fall: false,
            ams_enabled: false,
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
            electronics: Default::default(),
            spotter: None,
            spotter_events: Default::default(),
            tag: Default::default(),
            contacts: Default::default(),
            sensor_selection: Default::default(),
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
            dice: super::BattleDice::fresh(),
        })
    }

    /// Absolute facing of a surviving turret; its offset is attached to the hull.
    pub fn turret_heading(&self) -> Option<f64> {
        self.sections
            .get(&BattleVehicleSection::Turret)
            .filter(|state| state.internal > 0)
            .map(|_| {
                (self.motion.map_or(0.0, |motion| motion.heading) + self.turret_offset)
                    .rem_euclid(360.0)
            })
    }

    /// Current conditions used to suppress repeated hit-table effects.
    pub fn hit_condition(&self) -> super::BattleVehicleHitCondition {
        super::BattleVehicleHitCondition {
            immobilized: self.immobilized,
            turret_locked: self.turret_locked,
        }
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

    /// Disable propulsion, retaining the distinct standard/FASA and advanced immobility flags.
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
    pub fn crew_recovery(&self) -> &super::BattleRecovery {
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
        self.blinded_remaining = 0;
        self.hide_elapsed = None;
        self.cancel_digging();
        self.lose_vtol_lift();
        self.pilot = None;
        self.crew_recovery.clear();
        self.clear_fires();
        self.target_lock = None;
        self.power = super::BattlePower::Off;
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
            || self.definition.movement == super::BattleVehicleMovement::Stationary
        {
            return 0.0;
        }
        self.propulsion
            .maximum((self.definition.max_speed - self.motive_speed_loss).max(0.0))
    }

    /// Apply an explicit motive consequence; damage adapters own notices and the enclosing transaction.
    pub fn apply_motive_hit(&mut self, hit: super::BattleVehicleMotiveHit) {
        if self.immobilized
            || matches!(
                hit,
                super::BattleVehicleMotiveHit::SpeedLoss { movement_points: 0 }
            )
        {
            return;
        }
        match hit {
            super::BattleVehicleMotiveHit::Immobilize => self.immobilized = true,
            super::BattleVehicleMotiveHit::SpeedLoss { movement_points } => {
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
    pub fn character_pilot_status(&self) -> Option<super::BattleCharacterPilotStatus> {
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
    pub fn power(&self) -> super::BattlePower {
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
    pub fn free_fall(&self) -> Option<super::BattleFreeFall> {
        self.free_fall
            .or_else(|| self.vtol_flight.and_then(|flight| flight.fall))
    }

    /// Ground support height; hovercraft float at water level rather than on the bed.
    pub fn elevation_level(&self, tile: super::BattleHex) -> i32 {
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
    pub(super) fn terrain_elevation(&self, tile: super::BattleHex, under_bridge: bool) -> i32 {
        if under_bridge && tile.terrain == super::Terrain::Bridge {
            return 0;
        }
        if self.definition.movement == super::BattleVehicleMovement::Hover
            && matches!(tile.terrain, super::Terrain::Water | super::Terrain::Ice)
        {
            return 0;
        }
        i32::from(tile.standing_height())
    }

    /// Current continuous position and commanded motion.
    pub fn motion(&self) -> Option<super::BattleMotion> {
        self.motion
    }

    /// Commit a traced movement segment without resetting battlefield membership.
    pub(super) fn update_motion(
        &mut self,
        motion: super::BattleMotion,
        position: super::BattlePosition,
        under_bridge: bool,
    ) {
        if self.position != Some(position) {
            self.ground_elevation = None;
        }
        self.under_bridge = under_bridge;
        self.motion = Some(motion);
        self.position = Some(position);
    }

    /// Restore a rejected terrain entry while preserving any fall-induced heading and damage.
    pub(super) fn restore_ground_position(
        &mut self,
        position: super::BattlePosition,
        point: super::BattlePoint,
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
    pub fn position(&self) -> Option<super::BattlePosition> {
        (!self.detached).then_some(self.position).flatten()
    }

    /// Stable membership slot shared with other battlefield unit classes.
    pub fn map_slot(&self) -> Option<u32> {
        self.map_slot
    }

    /// Placement adapters update coordinates and slot together.
    pub(super) fn set_placement(&mut self, placement: Option<(super::BattlePosition, u32)>) {
        if self.position.map(|p| p.map) != placement.map(|(p, _)| p.map) {
            self.c3_network = None;
            self.tag.target = None;
            self.c3i_network = None;
        }
        self.under_bridge = false;
        self.ground_elevation = None;
        self.free_fall = None;
        self.orbital_drop = None;
        self.dig = super::BattleDigState::default();
        if let Some(flight) = &mut self.vtol_flight {
            *flight = super::BattleVtolFlight::default();
        }
        self.motion = placement.map(|(position, _)| {
            super::BattleMotion::stationary(
                super::BattleHexCoordinate {
                    x: i32::from(position.x),
                    y: i32::from(position.y),
                }
                .center(),
            )
        });
        self.detached = false;
        self.battlefield_label = None;
        self.position = placement.map(|(position, _)| position);
        self.map_slot = placement.map(|(_, slot)| slot);
    }

    /// Last physical coordinates remain available after removal from the tactical map.
    pub(super) fn retained_position(&self) -> Option<super::BattlePosition> {
        self.position
    }

    /// Remove map membership without discarding controls or the saved physical pose.
    pub(super) fn detach_scenario_membership(&mut self) {
        self.detached = self.position.is_some();
        self.map_slot = None;
        self.c3_network = None;
        self.tag.target = None;
        self.c3i_network = None;
        self.dig = super::BattleDigState::default();
        self.building_entry = None;
    }

    /// Replace tactical membership without resetting movement, altitude, flight or crew.
    pub(super) fn assign_membership(&mut self, position: super::BattlePosition, slot: u32) {
        self.detached = false;
        self.position = Some(position);
        self.map_slot = Some(slot);
        self.battlefield_label = None;
        self.c3_network = None;
        self.tag.target = None;
        self.c3i_network = None;
    }

    /// Immutable construction facts used for replay and equipment resolution.
    pub fn definition(&self) -> &BattleVehicleTemplate {
        &self.definition
    }

    /// Construction baseline used by the shared attacker movement calculation.
    pub fn template_speed(&self) -> f64 {
        super::template_speed::read(&self.definition.attributes, self.definition.max_speed)
            .expect("validated template speed")
    }

    /// Set the independent firing-movement baseline without changing propulsion.
    pub(super) fn set_template_speed(&mut self, speed: f64) {
        super::template_speed::write(&mut self.definition.attributes, speed);
    }

    /// Change only the authored engine allocation override; recalculation is a separate operation.
    pub(super) fn set_engine_sink_override(&mut self, value: i32) {
        super::engine_sink_override::write(&mut self.definition.attributes, value);
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
    pub(super) fn set_movement(&mut self, movement: super::BattleVehicleMovement) {
        let name = match movement {
            super::BattleVehicleMovement::Tracked => "Track",
            super::BattleVehicleMovement::Wheeled => "Wheel",
            super::BattleVehicleMovement::Hover => "Hover",
            super::BattleVehicleMovement::Stationary => "None",
            super::BattleVehicleMovement::Vtol => "VTOL",
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
    pub fn sections(&self) -> &BTreeMap<BattleVehicleSection, BattleSectionState> {
        &self.sections
    }

    /// Current rounds in the stable order of the resolved loadout.
    pub fn ammunition(&self) -> &[u16] {
        &self.ammunition
    }

    /// Resolve derived equipment without storing a second copy in the snapshot.
    pub fn loadout(&self) -> Result<BattleVehicleLoadout> {
        BattleVehicleLoadout::resolve(&self.definition)
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
            !matches!(
                section,
                BattleVehicleSection::Turret | BattleVehicleSection::Rotor
            ) && state.internal == 0
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
        draws: Vec<super::BattleAmmunitionDraw>,
    ) -> Vec<super::BattleAmmunitionDraw> {
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
        section: BattleVehicleSection,
        amount: u16,
        phase: BattleDamagePhase,
    ) -> Result<BattleDamageResult<BattleVehicleSection>> {
        ensure!(
            self.sections.contains_key(&section),
            "Vehicle section {} is absent",
            section.name()
        );
        let mut result = BattleDamageResult {
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
            BattleDamagePhase::Armor { .. } => &mut state.armor,
            BattleDamagePhase::Internal => &mut state.internal,
        };
        result.absorbed = (*protection).min(amount);
        *protection -= result.absorbed;
        if result.absorbed > 0 {
            self.live_mass.invalidate();
        }
        result.remaining -= result.absorbed;
        if matches!(phase, BattleDamagePhase::Internal) && state.internal == 0 {
            state.armor = 0;
            state.rear = 0;
            for (index, bin) in loadout.ammunition.iter().enumerate() {
                if bin.location.section == section {
                    self.ammunition[index] = 0;
                }
            }
            if section == BattleVehicleSection::Turret {
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
            if section == BattleVehicleSection::Rotor {
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

impl TryFrom<VehicleRecord> for BattleVehicle {
    type Error = anyhow::Error;

    /// Reject snapshots whose protection or ammunition cannot belong to the supplied construction.
    fn try_from(record: VehicleRecord) -> Result<Self> {
        let mut vehicle = Self::new(record.definition)?;
        ensure!(
            record.vtol_fuel.is_some() == vehicle.definition.is_vtol(),
            "Fuel state does not match vehicle class"
        );
        if let Some(fuel) = record.vtol_fuel {
            fuel.validate(&vehicle.definition)?;
        }
        vehicle.vtol_fuel = record.vtol_fuel;
        ensure!(
            record.vtol_flight.is_some() == vehicle.definition.is_vtol(),
            "Flight state does not match vehicle class"
        );
        if let Some(flight) = record.vtol_flight {
            flight.validate()?;
        }
        vehicle.vtol_flight = record.vtol_flight;
        ensure!(
            record.pilot_injuries <= i8::MAX as u8,
            "Invalid vehicle pilot injury count"
        );
        vehicle.pilot_injuries = record.pilot_injuries;
        vehicle.pilot_killed = record.pilot_killed;
        vehicle.character_pilot = record.character_pilot;
        ensure!(
            record.blinded_remaining <= 4,
            "Invalid sensor flash duration"
        );
        if let Some(timer) = record.self_destruct {
            timer.validate()?;
        }
        vehicle.self_destruct = record.self_destruct;
        vehicle.self_destruct_safe = record.self_destruct_safe;
        vehicle.blinded_remaining = record.blinded_remaining;
        ensure!(
            record.hide_elapsed.is_none_or(|elapsed| elapsed <= 100),
            "Invalid hiding timer"
        );
        vehicle.hide_elapsed = record.hide_elapsed;
        vehicle.crew_killed = record.crew_killed;
        vehicle.crew_recovery = record.crew_recovery;
        ensure!(
            (!record.crew_killed
                && !record.pilot_killed
                && !record.character_pilot.is_some_and(|status| status.killed))
                || (record.pilot.is_none()
                    && record.crew_stun_remaining == 0
                    && record.crew_stun_condition != Some(true)),
            "Dead vehicle crew retains pilot or stun"
        );
        ensure!(
            record.sections.keys().eq(vehicle.sections.keys()),
            "Vehicle snapshot sections do not match definition"
        );
        for (&section, state) in &record.sections {
            let original = &vehicle.sections[&section];
            ensure!(
                state.armor <= original.armor
                    && state.internal <= original.internal
                    && state.rear <= original.rear,
                "Vehicle snapshot protection exceeds definition in {}",
                section.name()
            );
            ensure!(
                state.internal > 0 || (state.armor == 0 && state.rear == 0),
                "Destroyed vehicle section retains armor"
            );
        }
        ensure!(
            record.lost_criticals.iter().all(|location| vehicle
                .definition
                .sections
                .get(&location.section)
                .is_some_and(|section| section.criticals.contains_key(&location.slot))),
            "Invalid destroyed vehicle equipment slot"
        );
        ensure!(
            record.piloting_damage <= 127 && record.gunnery_damage <= 127,
            "Invalid vehicle control penalty"
        );
        ensure!(
            record
                .lost_stabilizers
                .iter()
                .all(|section| vehicle.sections.contains_key(section)),
            "Invalid vehicle stabilizer section"
        );
        ensure!(
            record.crew_stun_remaining <= 60
                && (!record
                    .sections
                    .iter()
                    .any(|(section, state)| *section != BattleVehicleSection::Turret
                        && state.internal == 0)
                    || (record.crew_stun_remaining == 0
                        && record.crew_stun_condition != Some(true))),
            "Invalid vehicle crew stun countdown"
        );
        vehicle.crew_stun_remaining = record.crew_stun_remaining;
        vehicle.crew_stun_condition = record.crew_stun_condition;
        vehicle.piloting_damage = record.piloting_damage;
        vehicle.gunnery_damage = record.gunnery_damage;
        vehicle.lost_stabilizers = record.lost_stabilizers;
        ensure!(
            record
                .breached_sections
                .iter()
                .all(|section| record.sections.contains_key(section)),
            "Invalid breached vehicle section"
        );
        vehicle.breached_sections = record.breached_sections;
        vehicle.lost_criticals = record.lost_criticals;
        let loadout = vehicle.loadout()?;
        record.tics.validate(loadout.weapons.len())?;
        ensure!(
            record.ammunition.len() == loadout.ammunition.len(),
            "Vehicle snapshot ammunition bins do not match definition"
        );
        for (bin, &rounds) in loadout.ammunition.iter().zip(&record.ammunition) {
            ensure!(
                rounds <= bin.capacity,
                "Vehicle snapshot ammunition exceeds capacity"
            );
            ensure!(
                (record.sections[&bin.location.section].internal > 0
                    && !vehicle.critical_destroyed(bin.location))
                    || rounds == 0,
                "Destroyed vehicle section retains ammunition"
            );
        }
        ensure!(
            (!record.detached && record.position.is_some()) == record.map_slot.is_some(),
            "Vehicle position and battlefield slot must agree"
        );
        ensure!(
            !record.detached || record.position.is_some(),
            "Detached vehicle lacks retained coordinates"
        );
        if let Some(position) = record.position {
            ensure!(
                position.map.0 >= 0 && position.x <= 999 && position.y <= 999,
                "Invalid vehicle battlefield coordinates"
            );
        }
        if let super::BattlePower::Starting { remaining } = record.power {
            ensure!(
                (1..=30).contains(&remaining),
                "Invalid vehicle startup countdown"
            );
        }
        ensure!(
            record.power == super::BattlePower::Off || record.position.is_some(),
            "Powered vehicle requires a battlefield"
        );
        ensure!(
            record.motion.is_some() == record.position.is_some(),
            "Vehicle motion and position must agree"
        );
        ensure!(
            record.motive_speed_loss.is_finite()
                && record.motive_speed_loss >= 0.0
                && record.motive_speed_loss <= vehicle.definition.max_speed,
            "Invalid vehicle motive speed loss"
        );
        record.critical_conditions.validate(None)?;
        vehicle.critical_conditions = record.critical_conditions;
        record.live_mass.validate()?;
        vehicle.live_mass = record.live_mass;
        record.propulsion.validate()?;
        vehicle.propulsion = record.propulsion;
        vehicle.motive_speed_loss = record.motive_speed_loss;
        vehicle.immobilized = record.immobilized;
        vehicle.sprinting = record.sprinting;
        let maximum = super::sprint::saved_limit(vehicle.maximum_speed(), false, false, false);
        if let Some(motion) = record.motion {
            let propelled = motion.propelled(record.power)?;
            propelled.validate(maximum + 10.75)?;
            ensure!(
                vehicle.maximum_speed() > 0.0 || !propelled.active(),
                "Immobile vehicle retains motion"
            );
            ensure!(
                motion.desired_speed >= -maximum * 2.0 / 3.0 && motion.desired_speed <= maximum,
                "Invalid vehicle throttle"
            );
            let position = record.position.expect("validated placement");
            let coordinate = motion.point.containing_hex()?;
            ensure!(
                coordinate.x == i32::from(position.x) && coordinate.y == i32::from(position.y),
                "Vehicle continuous position differs from hex"
            );
            ensure!(
                record.power == super::BattlePower::Running
                    || !propelled.active()
                    || super::vtol_flight::idle_controls(
                        record.power,
                        record.vtol_flight,
                        record.motion
                    ),
                "Inactive vehicle retains motion"
            );
        }
        ensure!(
            !record.under_bridge
                || (record.position.is_some()
                    && vehicle.definition.movement == super::BattleVehicleMovement::Hover),
            "Invalid vehicle under-bridge state"
        );
        ensure!(
            record
                .ground_elevation
                .is_none_or(|height| record.position.is_some()
                    && height.is_finite()
                    && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&height)),
            "Retained elevation requires placement"
        );
        vehicle.free_fall = record.free_fall;
        vehicle.orbital_drop = record.orbital_drop;
        vehicle.ground_elevation = record.ground_elevation;
        vehicle.under_bridge = record.under_bridge;
        vehicle.dice = record.dice;
        vehicle.motion = record.motion;
        vehicle.pilot = record.pilot;
        vehicle.flooded = record.flooded;
        vehicle.transport_destroyed = record.transport_destroyed;
        ensure!(
            !record.tail_rotor_destroyed || vehicle.definition.is_vtol(),
            "Ground vehicle retains tail rotor damage"
        );
        vehicle.tail_rotor_destroyed = record.tail_rotor_destroyed;
        vehicle.power = record.power;
        vehicle.battlefield_label = record.battlefield_label;
        vehicle.preferred_id = record.preferred_id;
        vehicle.position = record.position;
        vehicle.detached = record.detached;
        vehicle.map_slot = record.map_slot;
        vehicle.sections = record.sections;
        ensure!(
            !vehicle.rotor_destroyed()
                || vehicle
                    .motion
                    .map(|motion| motion.propelled(vehicle.power))
                    .transpose()?
                    .is_none_or(|motion| !motion.active()),
            "Rotorless aircraft retains horizontal motion"
        );
        ensure!(
            record.turret_offset.is_finite() && (0.0..360.0).contains(&record.turret_offset),
            "Invalid vehicle turret facing"
        );
        ensure!(
            vehicle.turret_heading().is_some()
                || (!record.turret_locked && record.turret_offset == 0.0),
            "Missing turret retains facing or lock"
        );
        vehicle.turret_offset = record.turret_offset;
        vehicle.turret_locked = record.turret_locked;
        ensure!(
            !record.turret_jammed || vehicle.turret_heading().is_some(),
            "Invalid vehicle turret jam"
        );
        ensure!(
            record.turret_repairs.len() <= 256
                && record
                    .turret_repairs
                    .iter()
                    .all(|remaining| (1..=60).contains(remaining))
                && (record.turret_repairs.is_empty()
                    || (!vehicle.is_destroyed() && vehicle.turret_heading().is_some())),
            "Invalid vehicle turret repair countdown"
        );
        ensure!(
            !record.automatic_turret
                || vehicle
                    .definition
                    .sections
                    .contains_key(&BattleVehicleSection::Turret),
            "Automatic tracking without an authored turret"
        );
        vehicle.automatic_turret = record.automatic_turret;
        vehicle.turret_jammed = record.turret_jammed;
        vehicle.turret_repairs = record.turret_repairs;

        ensure!(
            record
                .weapon_recycle
                .iter()
                .all(
                    |(index, remaining)| loadout.weapons.get(*index).is_some_and(
                        |mount| *remaining > 0
                            && *remaining
                                <= if record.weapon_failures.contains_key(index) {
                                    120
                                } else {
                                    super::weapon_settings::MAX_RECYCLE_SECONDS
                                }
                            && !vehicle.critical_unavailable(mount.criticals[0])
                    )
                ),
            "Invalid vehicle weapon recycle timer"
        );
        ensure!(
            record.spent_launchers.iter().all(|index| loadout
                .weapons
                .get(*index)
                .is_some_and(|mount| mount.one_shot)),
            "Invalid spent vehicle launcher"
        );
        ensure!(
            loadout
                .weapons
                .iter()
                .enumerate()
                .all(|(index, mount)| !mount.initially_spent
                    || record.spent_launchers.contains(&index)),
            "Initially spent vehicle launcher was reloaded"
        );
        ensure!(
            record
                .fire_modes
                .iter()
                .all(|(index, mode)| *mode != super::BattleFireMode::Normal
                    && loadout
                        .weapons
                        .get(*index)
                        .is_some_and(|mount| mode.supports(mount.weapon))),
            "Invalid vehicle firing mode"
        );
        ensure!(
            record.ammunition_modes.iter().all(|(index, mode)| *mode
                != super::BattleAmmunitionMode::Normal
                && loadout
                    .weapons
                    .get(*index)
                    .is_some_and(|mount| mode.supports(mount.weapon))),
            "Invalid vehicle ammunition selection"
        );
        super::component_failure::validate(&record.component_failures, |location| {
            loadout.systems.iter().any(|part| part.location == location)
                && !super::damage_field::placeholder(
                    &vehicle.definition().sections[&location.section].criticals[&location.slot]
                        .equipment,
                )
        })?;
        vehicle.component_failures = record.component_failures;
        super::weapon_failure::validate(&record.weapon_failures, loadout.weapons.len(), |index| {
            !vehicle.critical_unavailable(loadout.weapons[index].criticals[0])
        })?;
        ensure!(
            record.jammed_weapons.iter().all(|index| loadout
                .weapons
                .get(*index)
                .is_some_and(|mount| mount.weapon.profile().ammunition_per_ton > 0
                    && !vehicle.critical_unavailable(mount.criticals[0]))),
            "Invalid vehicle ammunition-feed jam"
        );
        ensure!(
            record
                .unjam
                .is_none_or(|attempt| (1..=60).contains(&attempt.remaining)
                    && loadout
                        .weapons
                        .get(attempt.weapon_index)
                        .is_some_and(|mount| mount.weapon.profile().ammunition_per_ton > 0)),
            "Invalid vehicle unjam attempt"
        );
        vehicle.unjam = record.unjam;
        vehicle.jammed_weapons = record.jammed_weapons;
        super::weapon_power::validate(&record.powered_down_weapons, &loadout.weapons)?;
        vehicle.powered_down_weapons = record.powered_down_weapons;
        vehicle.weapon_failures = record.weapon_failures;
        ensure!(
            record
                .sensor_selection
                .pending
                .is_none_or(|pending| (1..=10).contains(&pending.remaining)),
            "Invalid vehicle sensor selection countdown"
        );
        ensure!(
            record
                .sensor_selection
                .active
                .supported_by_vehicle(&vehicle)
                && record
                    .sensor_selection
                    .pending
                    .is_none_or(|pending| pending.wanted.supported_by_vehicle(&vehicle)),
            "Vehicle sensor selection requires unavailable equipment"
        );
        vehicle.sensor_selection = record.sensor_selection;
        ensure!(
            record.target_lock.is_none_or(
                |lock| lock.remaining() <= 8 && vehicle.power == super::BattlePower::Running
            ),
            "Invalid vehicle target lock countdown or power state"
        );
        vehicle.friendly_fire_safety = record.friendly_fire_safety;
        vehicle.auto_fall = record.auto_fall;
        vehicle.target_lock = record.target_lock;
        if let Some(selection) = record.aimed_section {
            selection.validate()?;
        }
        vehicle.aimed_section = record.aimed_section;
        vehicle.artillery_adjustment = record.artillery_adjustment;
        ensure!(
            record.spotter.is_none_or(|id| id.0 > 0),
            "Invalid spotter identity"
        );
        vehicle.spotter = record.spotter;
        record.spotter_events.validate()?;
        vehicle.spotter_events = record.spotter_events;
        record.tag.validate()?;
        vehicle.tag = record.tag;
        ensure!(
            record.c3_network != Some(0) && record.c3i_network != Some(0),
            "Invalid vehicle command-network identity"
        );
        vehicle.c3_network = record.c3_network;
        vehicle.c3i_network = record.c3i_network;
        vehicle.electronics = record.electronics;
        vehicle.reconcile_electronics();
        ensure!(
            vehicle.electronics == record.electronics,
            "Invalid vehicle electronic emission state"
        );
        vehicle.contacts = record.contacts;
        record.brief.validate()?;
        vehicle.brief = record.brief;
        vehicle.tics = record.tics;
        vehicle.sensor_signature = record.sensor_signature;
        vehicle.scanner_perception = record.scanner_perception;
        super::radio::validate_channels(&record.radio)?;
        super::radio::validate_attributes(&vehicle.definition().attributes)?;
        ensure!(
            record.radio_experience_remaining <= 61,
            "Invalid radio experience countdown"
        );
        record.hardware.validate()?;
        vehicle.hardware = record.hardware;
        vehicle.cockpit_links = record.cockpit_links;
        vehicle.display_name = record.display_name;
        vehicle.markings = record.markings;
        vehicle.sixth_sense = record.sixth_sense;
        vehicle.last_startup = record.last_startup;
        vehicle.auxiliary_preferences = record.auxiliary_preferences;
        vehicle.base_movement_fields = record.base_movement_fields;
        vehicle.shot_counters = record.shot_counters;
        vehicle.damage_counters = record.damage_counters;
        vehicle.units_killed = record.units_killed;
        record
            .searchlight
            .validate(vehicle.definition().has_special("Searchlight"))?;
        vehicle.searchlight = record.searchlight;
        vehicle.tight_turn_mode = record.tight_turn_mode;
        vehicle.no_armor_warning = record.no_armor_warning;
        vehicle.no_ammunition_warning = record.no_ammunition_warning;
        vehicle.autocon_shutdown = record.autocon_shutdown;
        vehicle.searchlight_warning = record.searchlight_warning;
        vehicle.illumination_observed = record.illumination_observed;
        vehicle.radio = record.radio;
        vehicle.radio_skill = record.radio_skill;
        ensure!(
            record.sensor_signal.strength <= 100,
            "Invalid sensor signal"
        );
        vehicle.sensor_signal = record.sensor_signal;
        vehicle.fired_recently = record.fired_recently;
        vehicle.radio_experience_remaining = record.radio_experience_remaining;
        record.experience.validate()?;
        vehicle.experience = record.experience;
        vehicle.towable = record.towable;
        vehicle.fortified = record.fortified;
        vehicle.observer = record.observer;
        vehicle.weapons_hold = record.weapons_hold;
        vehicle.sprinting = record.sprinting;
        vehicle.combat_safe = record.combat_safe;
        vehicle.visibility = record.visibility;
        vehicle.dig = record.dig;
        ensure!(
            record
                .beacons
                .iter()
                .all(|(section, kinds)| !kinds.is_empty()
                    && vehicle
                        .sections
                        .get(section)
                        .is_some_and(|state| state.internal > 0)),
            "Vehicle beacon requires a surviving section and a nonempty effect set"
        );
        vehicle.beacons = record.beacons;
        ensure!(
            record
                .pod_removal
                .is_none_or(|remaining| (1..=60).contains(&remaining)),
            "Invalid vehicle pod-removal countdown"
        );
        vehicle.pod_removal = record.pod_removal;
        vehicle.ams_enabled = record.ams_enabled;
        ensure!(
            record.weapon_heat.is_finite(),
            "Invalid vehicle weapon heat"
        );
        vehicle.weapon_heat = record.weapon_heat;
        ensure!(
            record.inferno_remaining <= i32::MAX as u32,
            "Invalid inferno duration"
        );
        ensure!(
            record
                .burning_sections
                .iter()
                .all(
                    |(section, remaining)| vehicle.sections.contains_key(section)
                        && (1..=60).contains(remaining)
                ),
            "Invalid vehicle section fire"
        );
        ensure!(
            record
                .extinguishing
                .is_none_or(|remaining| (1..=120).contains(&remaining)),
            "Invalid vehicle extinguishing countdown"
        );
        vehicle.inferno_remaining = record.inferno_remaining;
        vehicle.burning_sections = record.burning_sections;
        vehicle.extinguishing = record.extinguishing;
        if let Some(entry) = record.building_entry {
            entry.validate()?;
        }
        vehicle.building_entry = record.building_entry;
        vehicle.fire_modes = record.fire_modes;
        super::ammunition_preference::validate(
            &record.ammunition_sections,
            &loadout.weapons,
            |section| {
                vehicle
                    .definition
                    .sections
                    .get(&section)
                    .is_some_and(|s| s.internal > 0)
            },
        )?;
        vehicle.ammunition_sections = record.ammunition_sections;
        vehicle.ammunition_modes = record.ammunition_modes;
        vehicle.weapon_recycle = record.weapon_recycle;
        vehicle.spent_launchers = record.spent_launchers;
        vehicle.ammunition = record.ammunition;
        ensure!(
            !vehicle.is_destroyed() || vehicle.power == super::BattlePower::Off,
            "Destroyed vehicle must be shut down"
        );
        super::crew_recovery::validate(
            &vehicle.crew_recovery,
            vehicle.pilot(),
            vehicle.pilot_injuries(),
        )?;
        vehicle.validate_ground_descent()?;
        vehicle.validate_orbital_drop()?;
        vehicle.validate_dig()?;
        Ok(vehicle)
    }
}

impl BattleVehicle {
    /// Common identity projection for object registration and saved-game inspection.
    pub(crate) fn identity(&self) -> super::StoredBattleUnit {
        super::StoredBattleUnit {
            name: self.definition.name.clone(),
            template: self.definition.reference.clone(),
            class_code: if self.definition.is_vtol() { 2 } else { 1 },
            movement_code: match self.definition.movement {
                super::BattleVehicleMovement::Tracked => 1,
                super::BattleVehicleMovement::Wheeled => 2,
                super::BattleVehicleMovement::Hover => 3,
                super::BattleVehicleMovement::Stationary => 10,
                super::BattleVehicleMovement::Vtol => 4,
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
    definition: BattleVehicleTemplate,
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
    let vehicle = BattleVehicle::new(definition)?;
    std::sync::Arc::make_mut(&mut world.btech.units).insert(id, vehicle.identity());
    std::sync::Arc::make_mut(&mut world.btech.vehicles).insert(id, vehicle);
    std::sync::Arc::make_mut(&mut world.btech.registrations).insert(id, "MECH".into());
    Ok(())
}

/// Apply a vehicle material phase in a world candidate; the combat caller owns subsequent effects.
pub fn damage_vehicle_phase(
    world: &mut crate::World,
    id: crate::ObjectId,
    section: BattleVehicleSection,
    amount: u16,
    phase: BattleDamagePhase,
) -> Result<BattleDamageResult<BattleVehicleSection>> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Vehicle object is unavailable"
    );
    std::sync::Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .context("Constructed vehicle not found")?
        .damage_phase(section, amount, phase)
}

/// Apply vehicle motive damage inside the caller's world candidate; adapters own notifications.
pub fn damage_vehicle_motive(
    world: &mut crate::World,
    id: crate::ObjectId,
    hit: super::BattleVehicleMotiveHit,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Vehicle object is unavailable"
    );
    let vehicle = std::sync::Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .context("Constructed vehicle not found")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is already destroyed");
    vehicle.apply_motive_hit(hit);
    Ok(())
}
