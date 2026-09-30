//! Constructed BattleMechs with an owned definition and mutable armor/ammunition state.
use super::{BattleLoadout, BattleSection, BattleSystem, BattleTemplate, StoredBattleUnit};
use crate::ObjectId;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Toggle one technology flag in the named specials attribute ("specials",
/// "specials2", or "infantry_specials"), mirroring the native per-group flags.
pub(super) fn edit_special(
    attributes: &mut BTreeMap<String, String>,
    attribute: &str,
    flag: &str,
    enabled: bool,
) {
    let mut values: Vec<String> = attributes
        .get(attribute)
        .into_iter()
        .flat_map(|value| value.split_ascii_whitespace())
        .filter(|value| *value != "-" && !value.eq_ignore_ascii_case(flag))
        .map(str::to_owned)
        .collect();
    if enabled {
        values.push(flag.to_owned());
    }
    attributes.insert(
        attribute.into(),
        if values.is_empty() {
            "-".into()
        } else {
            values.join(" ")
        },
    );
}

/// Ground hex occupied by a unit; coordinates are zero-based columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattlePosition {
    pub map: ObjectId,
    pub x: u16,
    pub y: u16,
}

/// Remaining protection on one section of a constructed combat unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleSectionState {
    pub armor: u16,
    pub internal: u16,
    pub rear: u16,
}

/// Persistent construction state; movement and combat transitions are added to this domain model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleUnit {
    #[serde(default)]
    pub(super) propulsion: super::propulsion::Propulsion,
    #[serde(default)]
    pub(super) live_mass: super::live_mass::LiveMass,
    #[serde(default)]
    pub(super) critical_conditions: super::critical_conditions::CriticalConditions,
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
    #[serde(default)]
    pub(super) display_name: super::display_name::DisplayName,
    #[serde(default)]
    pub(super) markings: super::markings::Markings,
    #[serde(default)]
    pub(super) sixth_sense: super::sixth_sense::SixthSense,
    #[serde(default)]
    pub(super) last_startup: i64,
    #[serde(default)]
    pub(super) cockpit_links: super::cockpit_links::CockpitLinks,
    #[serde(default)]
    pub(super) last_jump: super::jumping::LastJump,
    #[serde(default)]
    pub(super) hardware: super::hardware_settings::HardwareSettings,
    /// Remaining eligibility after the first pristine center-torso internal hit.
    #[serde(default)]
    pub(super) reactor_instability_remaining: Option<u8>,
    #[serde(default)]
    pub(super) self_destruct_safe: bool,
    #[serde(default)]
    pub(super) self_destruct: Option<super::BattleSelfDestruct>,
    /// Elapsed one-second camouflage preparation checks.
    #[serde(default)]
    pub(super) hide_elapsed: Option<u16>,
    /// Scenario permission to tow this unit outside in-character gameplay.
    #[serde(default)]
    pub(super) towable: bool,
    /// Scenario emplacement; movement and towing share the fortification gate.
    #[serde(default)]
    pub(super) fortified: bool,
    /// Hull disabled by destruction of its carrier; crew and material remain independent.
    pub(super) transport_destroyed: bool,
    /// Opt in to routine notices about shutdown contacts.
    #[serde(default)]
    pub(super) autocon_shutdown: bool,
    /// Contact presentation and routine-notice choices.
    #[serde(default)]
    pub(super) brief: super::BattleBriefSettings,
    #[serde(default)]
    pub(super) tics: super::BattleTics,
    #[serde(default)]
    pub(super) lateral: super::BattleLateralState,
    #[serde(default)]
    pub(super) hull_down: super::BattleHullDownState,
    #[serde(default)]
    pub(super) masc: super::BattleBoosterState,
    /// Shared identity of a C3i network; peers are derived from world membership.
    #[serde(default)]
    pub(super) c3i_network: Option<u64>,
    /// Classic C3 membership is independent of C3i.
    #[serde(default)]
    pub(super) c3_network: Option<u64>,
    /// Which network families the server may link automatically.
    #[serde(default)]
    pub(super) network_automation: super::BattleNetworkAutomation,
    #[serde(default)]
    pub(super) supercharger: super::BattleBoosterState,
    /// Resolve simultaneous booster checks in the order their timers were scheduled.
    #[serde(default)]
    pub(super) supercharger_scheduled_last: bool,
    /// Channel slots remain saved independently of active hardware limits.
    #[serde(default)]
    pub(super) radio: [super::BattleRadioChannel; 16],
    /// Communication target captured when startup completes.
    #[serde(default = "super::radio::default_skill")]
    pub(super) radio_skill: i16,
    /// Saved interval gate for communication experience attempts.
    #[serde(default)]
    pub(super) radio_experience_remaining: u8,
    /// Administrator-assigned observer role.
    #[serde(default)]
    pub(super) observer: bool,
    /// Operator-imposed firing restriction, separate from weapon mechanics.
    #[serde(default)]
    pub(super) weapons_hold: bool,
    /// Operator-imposed immunity to combat damage.
    #[serde(default)]
    pub(super) combat_safe: bool,
    /// Operator visibility privileges, independent of sensor equipment.
    #[serde(default)]
    pub(super) visibility: super::BattleVisibility,
    /// Selected observer, or this unit itself while spotting.
    #[serde(default)]
    pub(super) spotter: Option<crate::ObjectId>,
    #[serde(default)]
    pub(super) spotter_events: super::BattleSpotterEvents,
    /// Saved correction for the current artillery target.
    #[serde(default)]
    pub(super) artillery_adjustment: u8,
    /// Owned TAG selection and lock/recycle countdown.
    #[serde(default)]
    pub(super) tag: super::BattleTagState,
    /// Recent weapon emission, cleared by the next committed heartbeat.
    #[serde(default)]
    pub(super) fired_recently: bool,
    /// Null signature controls and persisted switch destination.
    #[serde(default)]
    pub(super) null_signature: super::BattleSignatureState,
    /// Owned stealth armor selection and switch countdown.
    #[serde(default)]
    pub(super) stealth: super::BattleSignatureState,
    /// Exclusive suite controls and committed field observations.
    #[serde(default)]
    pub(super) electronics: super::BattleElectronics,
    /// Conventional and iNarc pod effects attached to surviving sections.
    #[serde(default)]
    pub(super) beacons:
        BTreeMap<BattleSection, std::collections::BTreeSet<super::BattleBeaconKind>>,
    /// Pilot-selected automatic anti-missile defense.
    #[serde(default)]
    pub(super) ams_enabled: bool,
    #[serde(default)]
    pub(super) flight: Option<super::BattleJumpFlight>,
    #[serde(default)]
    pub(super) free_fall: Option<super::BattleFreeFall>,
    /// Cocoon or jump-jet descent owns altitude independently of engine power.
    #[serde(default)]
    pub(super) orbital_drop: Option<super::BattleOrbitalDrop>,
    /// Explicit altitude retained when the supporting terrain changes.
    #[serde(default)]
    pub(super) ground_elevation: Option<f64>,
    /// A bridge collision interrupted the transition from continuous position to tactical hex.
    #[serde(default)]
    pub(super) hex_sync_pending: bool,
    /// Skip downhill cliff avoidance while a pilot is assigned.
    #[serde(default)]
    pub(super) auto_fall: bool,
    /// Remaining committed seconds for physical limb recovery.
    #[serde(default)]
    pub(super) limb_recycle: BTreeMap<BattleSection, u16>,
    /// Arm holding a tree for a later two-handed club attack.
    #[serde(default)]
    pub(super) carried_club: Option<super::BattleArm>,
    /// Charge selection and accumulated ground movement.
    #[serde(default)]
    pub(super) charge: super::BattleChargeState,
    /// Reject weapon fire on units with the same team identifier.
    #[serde(default)]
    pub(super) friendly_fire_safety: bool,
    /// Opt-out flags preserve enabled-by-default combat warnings.
    #[serde(default)]
    pub(super) no_armor_warning: bool,
    #[serde(default)]
    pub(super) no_ammunition_warning: bool,
    /// Opt in to notifications when another unit illuminates this unit.
    #[serde(default)]
    pub(super) searchlight_warning: bool,
    /// Last committed notification observation; never used for sensor eligibility.
    #[serde(default)]
    pub(super) illumination_observed: bool,
    #[serde(default)]
    pub(super) jump_stabilization: u8,
    #[serde(default)]
    pub(super) flooded_sections: std::collections::BTreeSet<BattleSection>,
    /// Sections disabled by vacuum exposure, independent of flooding.
    #[serde(default)]
    pub(super) breached_sections: std::collections::BTreeSet<BattleSection>,
    #[serde(default)]
    pub(super) stagger: super::BattleStagger,
    #[serde(default)]
    pub(super) posture: super::BattlePosture,
    #[serde(default)]
    pub(super) building_entry: Option<super::BattleBuildingEntry>,
    #[serde(default)]
    pub(super) stand_timer: Option<super::BattleStandTimer>,
    #[serde(default)]
    pub(super) signature: super::BattleUnitSignature,
    #[serde(default = "super::scanner::default_perception")]
    pub(super) scanner_perception: i16,
    #[serde(default)]
    pub(super) contacts: BTreeMap<ObjectId, super::BattleContact>,
    #[serde(default)]
    pub(super) target_lock: Option<super::BattleTargetSelection>,
    #[serde(default)]
    pub(super) aimed_section: Option<super::BattleAimSelection>,
    #[serde(default)]
    pub(super) searchlight: super::BattleSearchlight,
    #[serde(default)]
    pub(super) facing: super::BattleFacing,
    #[serde(default)]
    pub(super) stun_remaining: u8,
    #[serde(default)]
    pub(super) pilot_injuries: u8,
    /// Confirmed tactical pilot death, distinct from a startup health-derived count.
    #[serde(default)]
    pub(super) pilot_killed: bool,
    /// Tactical recovery owned by an unoccupied cockpit.
    pub(super) crew_recovery: super::BattleRecovery,
    #[serde(default)]
    pub(super) character_pilot: Option<super::BattleCharacterPilotStatus>,
    #[serde(default)]
    pub(super) experience: super::BattleUnitExperience,
    /// Committed movement cadence and piloting XP coordinate mark.
    #[serde(default)]
    pub(super) movement_experience: super::BattleMovementExperience,
    #[serde(default)]
    pub(super) heat: super::BattleHeat,
    #[serde(default)]
    pub(super) heat_sample: super::BattleHeatRates,
    #[serde(default)]
    pub(super) heat_cutoff: super::BattleHeatCutoff,
    /// Total cooling adopted at the last material reconstruction, before physical sink losses.
    #[serde(default)]
    pub(super) reconstructed_cooling: Option<u16>,
    /// Saved seconds of inferno burning, independent of reactor power.
    #[serde(default)]
    pub(super) inferno_remaining: u32,
    #[serde(default)]
    pub(super) overheat_clock: super::BattleOverheatClock,
    #[serde(default)]
    pub(super) weapon_recycle: BTreeMap<usize, u16>,
    /// Gauss mounts deliberately powered down; independent of material damage.
    #[serde(default)]
    pub(super) powered_down_weapons: std::collections::BTreeSet<usize>,
    #[serde(default)]
    pub(super) jammed_weapons: std::collections::BTreeSet<usize>,
    #[serde(default)]
    pub(super) unjam: Option<super::BattleUnjam>,
    #[serde(default)]
    pub(super) dumping: Option<super::BattleDump>,
    /// Indices of self-contained launchers whose salvo has been expended.
    #[serde(default)]
    pub(super) spent_launchers: std::collections::BTreeSet<usize>,
    #[serde(default)]
    pub(super) fire_modes: BTreeMap<usize, super::BattleFireMode>,
    #[serde(default)]
    pub(super) ammunition_modes: BTreeMap<usize, super::BattleAmmunitionMode>,
    #[serde(default)]
    pub(super) ammunition_sections: BTreeMap<usize, BattleSection>,
    #[serde(default)]
    pub(super) weapon_damage: Vec<super::BattleWeaponDamage>,
    /// Inspectable component conditions independent of physical system availability.
    #[serde(default)]
    pub(super) component_failures: Vec<super::BattleComponentFailure<super::CriticalLocation>>,
    /// Temporary weapon conditions independent of material loss and recovery timers.
    #[serde(default)]
    pub(super) weapon_failures: BTreeMap<usize, super::BattleEquipmentFailure>,
    #[serde(default)]
    pub(super) weapon_damage_jams: std::collections::BTreeSet<usize>,
    #[serde(default)]
    pub(super) lost_criticals: std::collections::BTreeSet<super::CriticalLocation>,
    pub(crate) dice: super::BattleDice,
    #[serde(default)]
    pub(crate) motion: Option<super::BattleMotion>,
    #[serde(default)]
    pub(crate) power: super::BattlePower,
    #[serde(default)]
    pub(crate) pilot: Option<ObjectId>,
    #[serde(default)]
    pub(crate) position: Option<BattlePosition>,
    /// Tactical membership may be removed while the last physical pose is retained.
    #[serde(default)]
    pub(super) detached: bool,
    /// Persisted map membership slot; absent exactly when the unit is unplaced.
    #[serde(default)]
    pub(super) map_slot: Option<u32>,
    /// Scenario-selected identity, independent of battlefield membership order.
    #[serde(default)]
    pub(super) battlefield_label: Option<String>,
    /// Optional configured identity, used only when selecting a new battlefield ID.
    #[serde(default)]
    pub(super) preferred_id: Option<super::BattlePreferredId>,
    #[serde(default)]
    contract_loadout: bool,
    #[serde(default)]
    administrative_raw: Option<super::AdministrativeRawUnit>,
    definition: BattleTemplate,
    pub(super) sections: BTreeMap<BattleSection, BattleSectionState>,
    pub(super) ammunition: Vec<u16>,
}

// Live fields change routinely while a unit moves, heats, fires and recovers; everything
// else is construction, damage and settings, saved only when it changes.
super::saved_parts::saved_parts!(BattleUnit {
    core: [
        propulsion,
        live_mass,
        critical_conditions,
        auxiliary_preferences,
        base_movement_fields,
        units_killed,
        display_name,
        markings,
        sixth_sense,
        cockpit_links,
        last_jump,
        hardware,
        self_destruct_safe,
        self_destruct,
        towable,
        fortified,
        transport_destroyed,
        autocon_shutdown,
        brief,
        tics,
        lateral,
        hull_down,
        c3i_network,
        c3_network,
        network_automation,
        radio,
        radio_skill,
        radio_experience_remaining,
        observer,
        weapons_hold,
        combat_safe,
        visibility,
        spotter,
        artillery_adjustment,
        tag,
        null_signature,
        stealth,
        electronics,
        beacons,
        ams_enabled,
        orbital_drop,
        auto_fall,
        carried_club,
        friendly_fire_safety,
        no_armor_warning,
        no_ammunition_warning,
        searchlight_warning,
        illumination_observed,
        flooded_sections,
        breached_sections,
        posture,
        building_entry,
        signature,
        scanner_perception,
        contacts,
        searchlight,
        pilot_injuries,
        pilot_killed,
        crew_recovery,
        character_pilot,
        experience,
        movement_experience,
        heat_cutoff,
        reconstructed_cooling,
        powered_down_weapons,
        jammed_weapons,
        spent_launchers,
        fire_modes,
        ammunition_modes,
        ammunition_sections,
        weapon_damage,
        component_failures,
        weapon_failures,
        weapon_damage_jams,
        lost_criticals,
        pilot,
        detached,
        map_slot,
        battlefield_label,
        preferred_id,
        contract_loadout,
        administrative_raw,
        definition,
        sections,
    ],
    live: [
        motion,
        position,
        facing,
        power,
        heat,
        heat_sample,
        overheat_clock,
        inferno_remaining,
        target_lock,
        aimed_section,
        stagger,
        stand_timer,
        weapon_recycle,
        limb_recycle,
        fired_recently,
        shot_counters,
        damage_counters,
        last_startup,
        reactor_instability_remaining,
        stun_remaining,
        flight,
        free_fall,
        charge,
        jump_stabilization,
        hide_elapsed,
        ground_elevation,
        hex_sync_pending,
        masc,
        supercharger,
        supercharger_scheduled_last,
        dumping,
        unjam,
        spotter_events,
    ],
    live_always: [dice, ammunition,],
});

impl BattleUnit {
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
            .ok_or_else(|| anyhow::anyhow!("weapon number is not mounted"))?;
        if fire.iter().any(|mode| mode == "Destroyed") {
            self.lost_criticals.insert(first);
        } else {
            self.lost_criticals.remove(&first);
        }
        let failure = if fire.iter().any(|mode| mode == "Disabled") {
            Some(super::BattleEquipmentFailure::Disabled)
        } else if fire.iter().any(|mode| mode == "Broken") {
            Some(super::BattleEquipmentFailure::Dud)
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
        replace_critical_modes(modes, fire, ammunition);
        Ok(())
    }
    pub(super) fn set_administrative_heat_sinks(&mut self, count: u16) {
        self.definition.heat_sinks = count;
        self.reconstructed_cooling = None;
    }

    pub(super) fn set_administrative_special(
        &mut self,
        attribute: &str,
        flag: &str,
        enabled: bool,
    ) {
        edit_special(&mut self.definition.attributes, attribute, flag, enabled);
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
        section: BattleSection,
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
        section: BattleSection,
        kind: super::AdministrativeRepairKind,
        value: u16,
        hull: super::ReattachHull,
    ) -> Result<()> {
        if kind == super::AdministrativeRepairKind::Part {
            self.repair_critical(super::CriticalLocation {
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
                    self.flooded_sections.remove(&section);
                }
            }
            super::AdministrativeRepairKind::Part => unreachable!(),
        }
        Ok(())
    }

    fn repair_critical(&mut self, location: super::CriticalLocation) -> Result<()> {
        // Failure/mode bits can make an authored weapon run deliberately inconsistent
        // across its criticals. Resolve a diagnostic copy with only the transient C
        // repair bits removed so we can identify the affected mount before repairing
        // the selected raw slot.
        let mut resolvable = self.definition.clone();
        for section in resolvable.sections.values_mut() {
            for critical in section.criticals.values_mut() {
                if super::equipment::strip_name_prefix(&critical.equipment, "IS.").is_some()
                    || super::equipment::strip_name_prefix(&critical.equipment, "CL.").is_some()
                {
                    // C stores mount modes on the first critical while the remaining
                    // slots can retain older raw values. Emptying weapon modes in the
                    // diagnostic copy keeps grouping based on equipment/data/brand.
                    critical.modes.clear();
                }
            }
        }
        let loadout = if self.contract_loadout {
            BattleLoadout::resolve_contract(&resolvable)?
        } else {
            BattleLoadout::resolve(&resolvable)?
        };
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
        critical.brand = critical.brand.map(|brand| brand % 16);
        if weapon.is_some() || ammunition.is_some() {
            critical.data = "0".into();
        }
        self.lost_criticals.remove(&location);
        self.component_failures
            .retain(|failure| failure.location != location);
        self.weapon_damage
            .retain(|damage| damage.location != location);
        if let Some(index) = weapon {
            self.weapon_failures.remove(&index);
            self.weapon_damage_jams.remove(&index);
            self.jammed_weapons.remove(&index);
            self.spent_launchers.remove(&index);
            self.powered_down_weapons.remove(&index);
            self.weapon_recycle.remove(&index);
        }
        if let Some(index) = ammunition {
            self.ammunition[index] = 0;
        }
        // C repair writes raw zero auxiliary data for weapon/ammunition slots.
        // That representation is intentionally broader than strict authored MML.
        self.contract_loadout = true;
        // C's do_magic rebuilds equipment-derived state for repaired special criticals.
        self.reconstructed_cooling = None;
        self.live_mass.invalidate();
        Ok(())
    }

    pub(super) fn replace_construction_contract(
        &mut self,
        definition: BattleTemplate,
        touched: &[super::CriticalLocation],
    ) -> Result<()> {
        self.replace_construction_mode(definition, touched, true)
    }

    fn replace_construction_mode(
        &mut self,
        definition: BattleTemplate,
        touched: &[super::CriticalLocation],
        contract: bool,
    ) -> Result<()> {
        let old = self.loadout()?.clone();
        let replacement = if contract {
            Self::from_contract_template(definition)?
        } else {
            Self::from_template(definition)?
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
        self.fire_modes = remap_map(&remap, &self.fire_modes);
        self.ammunition_modes = remap_map(&remap, &self.ammunition_modes);
        self.ammunition_sections = remap_map(&remap, &self.ammunition_sections);
        self.weapon_recycle = remap_map(&remap, &self.weapon_recycle);
        self.jammed_weapons = remap_set(&remap, &self.jammed_weapons);
        self.spent_launchers = remap_set(&remap, &self.spent_launchers);
        self.powered_down_weapons = remap_set(&remap, &self.powered_down_weapons);
        self.weapon_failures = remap_map(&remap, &self.weapon_failures);
        self.weapon_damage_jams = remap_set(&remap, &self.weapon_damage_jams);
        self.lost_criticals.retain(|slot| !touched.contains(slot));
        self.weapon_damage
            .retain(|damage| !touched.contains(&damage.location));
        self.component_failures
            .retain(|failure| !touched.contains(&failure.location));
        self.live_mass.invalidate();
        Ok(())
    }

    /// Saved character-mode injury count and fatal status, separate from tactical injury rules.
    pub fn character_pilot_status(&self) -> Option<super::BattleCharacterPilotStatus> {
        self.character_pilot
    }

    /// Saved virtual-crew recovery; a present pilot owns their personal recovery instead.
    pub fn crew_recovery(&self) -> &super::BattleRecovery {
        &self.crew_recovery
    }

    /// Current cockpit injury count; confirmed pilot death is an independent event.
    pub fn pilot_injuries(&self) -> u8 {
        self.pilot_injuries
    }

    /// Continuous motion, defaulting to the placed hex center before its first update.
    pub fn motion(&self) -> Option<super::BattleMotion> {
        self.motion.or_else(|| {
            self.position.map(|position| {
                super::BattleMotion::stationary(
                    super::BattleHexCoordinate {
                        x: i32::from(position.x),
                        y: i32::from(position.y),
                    }
                    .center(),
                )
            })
        })
    }

    /// Unix time supplied at the most recent completed startup, initially zero.
    pub fn last_startup(&self) -> i64 {
        self.last_startup
    }

    /// Current engine state and pending startup countdown.
    pub fn power(&self) -> super::BattlePower {
        self.power
    }

    /// Player currently occupying the cockpit, independent of passengers inside the unit.
    pub fn pilot(&self) -> Option<ObjectId> {
        self.pilot
    }

    /// Whether the pilot has opted to fall off cliffs without an avoidance check.
    pub fn auto_fall(&self) -> bool {
        self.auto_fall
    }

    /// Whether a collision stopped movement before its tactical hex could be updated.
    pub fn hex_sync_pending(&self) -> bool {
        self.hex_sync_pending
    }

    /// Current battlefield coordinates, absent when the unit is off-map.
    pub fn position(&self) -> Option<BattlePosition> {
        (!self.detached).then_some(self.position).flatten()
    }

    /// Construct an undamaged conventional biped from a fully resolved supported definition.
    pub fn from_template(definition: BattleTemplate) -> Result<Self> {
        Self::from_template_mode(definition, false)
    }

    /// Admit the broader raw critical layouts accepted by the canonical C administrator.
    pub(crate) fn from_contract_template(definition: BattleTemplate) -> Result<Self> {
        Self::from_template_mode(definition, true)
    }

    fn from_template_mode(mut definition: BattleTemplate, contract_loadout: bool) -> Result<Self> {
        if contract_loadout {
            super::template_ammunition::normalize_contract(&mut definition)?;
        } else {
            super::template_ammunition::normalize(&mut definition)?;
        }
        // The tonnage-chart internal structure (mech_int_check) is forced while
        // the template file is read; construction and saved-definition restore
        // keep the stored internals verbatim.
        if !contract_loadout {
            validate_definition(&definition)?;
        }
        let loadout = if contract_loadout {
            BattleLoadout::resolve_contract(&definition)?
        } else {
            BattleLoadout::resolve(&definition)?
        };
        let sections = definition
            .sections
            .iter()
            .map(|(&section, original)| {
                (
                    section,
                    BattleSectionState {
                        armor: original.armor,
                        internal: original.internal,
                        rear: original.rear,
                    },
                )
            })
            .collect();
        let ammunition = loadout.ammunition.iter().map(|bin| bin.rounds).collect();
        let unit = Self {
            propulsion: Default::default(),
            live_mass: Default::default(),
            critical_conditions: Default::default(),
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
            last_jump: Default::default(),
            hardware: Default::default(),
            autocon_shutdown: false,
            brief: Default::default(),
            tics: Default::default(),
            lateral: Default::default(),
            hull_down: Default::default(),
            masc: Default::default(),
            c3i_network: None,
            c3_network: None,
            network_automation: Default::default(),
            supercharger: Default::default(),
            supercharger_scheduled_last: false,
            radio: Default::default(),
            radio_skill: super::radio::default_skill(),
            radio_experience_remaining: 0,
            observer: false,
            weapons_hold: false,
            combat_safe: false,
            visibility: super::BattleVisibility::default(),
            fired_recently: false,
            null_signature: Default::default(),
            stealth: Default::default(),
            electronics: Default::default(),
            beacons: Default::default(),
            // Anti-missile defense is purely reactive, so installed systems start armed.
            ams_enabled: loadout.weapons.iter().any(|mount| mount.weapon.is_ams()),
            flight: None,
            free_fall: None,
            orbital_drop: None,
            ground_elevation: None,
            hex_sync_pending: false,
            auto_fall: false,
            limb_recycle: Default::default(),
            carried_club: None,
            charge: Default::default(),
            friendly_fire_safety: false,
            no_armor_warning: false,
            no_ammunition_warning: false,
            searchlight_warning: false,
            illumination_observed: false,
            jump_stabilization: 0,
            transport_destroyed: false,
            flooded_sections: Default::default(),
            breached_sections: Default::default(),
            stagger: Default::default(),
            tag: Default::default(),
            spotter: None,
            spotter_events: Default::default(),
            artillery_adjustment: 0,
            signature: Default::default(),
            scanner_perception: super::scanner::default_perception(),
            posture: Default::default(),
            stand_timer: None,
            building_entry: None,
            contacts: Default::default(),
            target_lock: None,
            aimed_section: None,
            searchlight: Default::default(),
            facing: Default::default(),
            stun_remaining: 0,
            pilot_injuries: 0,
            pilot_killed: false,
            crew_recovery: super::BattleRecovery::fresh(),
            character_pilot: None,
            towable: false,
            fortified: false,
            experience: Default::default(),
            movement_experience: Default::default(),
            heat: Default::default(),
            heat_sample: Default::default(),
            heat_cutoff: Default::default(),
            reconstructed_cooling: None,
            hide_elapsed: None,
            self_destruct: None,
            reactor_instability_remaining: None,
            self_destruct_safe: false,
            inferno_remaining: 0,
            overheat_clock: Default::default(),
            ammunition_sections: BTreeMap::new(),
            fire_modes: loadout
                .weapons
                .iter()
                .enumerate()
                .filter(|(_, mount)| mount.initial_fire_mode != super::BattleFireMode::Normal)
                .map(|(index, mount)| (index, mount.initial_fire_mode))
                .collect(),
            ammunition_modes: loadout
                .weapons
                .iter()
                .enumerate()
                .filter(|(_, mount)| {
                    mount.initial_ammunition_mode != super::BattleAmmunitionMode::Normal
                })
                .map(|(index, mount)| (index, mount.initial_ammunition_mode))
                .collect(),
            weapon_recycle: BTreeMap::new(),
            jammed_weapons: Default::default(),
            unjam: None,
            dumping: None,
            spent_launchers: loadout
                .weapons
                .iter()
                .enumerate()
                .filter_map(|(index, mount)| mount.initially_spent.then_some(index))
                .collect(),
            lost_criticals: Default::default(),
            weapon_damage: Default::default(),
            powered_down_weapons: Default::default(),
            component_failures: Default::default(),
            weapon_failures: Default::default(),
            weapon_damage_jams: Default::default(),
            dice: super::BattleDice::fresh(),
            motion: None,
            power: super::BattlePower::Off,
            pilot: None,
            position: None,
            detached: false,
            map_slot: None,
            battlefield_label: None,
            preferred_id: None,
            contract_loadout,
            administrative_raw: None,
            definition,
            sections,
            ammunition,
        };
        if !contract_loadout
            && unit
                .definition
                .attributes
                .get("cargo_space")
                .is_some_and(|value| value != "0")
        {
            unit.mass()?;
        }
        Ok(unit)
    }

    /// The persisted definition, independent of subsequent source-asset changes.
    pub fn definition(&self) -> &BattleTemplate {
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

    /// Change limb roles while retaining their installed equipment and damage.
    pub(super) fn set_chassis(&mut self, chassis: super::BattleMechChassis) {
        let name = match chassis {
            super::BattleMechChassis::Biped => "Biped",
            super::BattleMechChassis::Quad => "Quad",
        };
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

    /// Current section armor and internal structure.
    pub fn sections(&self) -> &BTreeMap<BattleSection, BattleSectionState> {
        &self.sections
    }

    /// Remaining complete salvos, ordered by the resolved ammunition bins.
    pub fn ammunition(&self) -> &[u16] {
        &self.ammunition
    }

    /// Inspect typed equipment from the owned definition.
    pub fn loadout(&self) -> Result<BattleLoadout> {
        if let Some(projection) = super::loadout_context::mech(self) {
            return Ok(projection);
        }
        let projection = super::equipment_context::mech(&self.definition, self.contract_loadout)?;
        super::loadout_context::remember_mech(self, &projection);
        Ok(projection)
    }

    /// Reject corrupt or unsupported persisted construction state.
    pub(crate) fn validate(&self) -> Result<()> {
        let _measurement = super::autopilot::diagnostics::combat("validation_unit");
        let _loadouts = super::loadout_context::LoadoutScope::unit(self);
        self.last_jump.validate()?;
        self.propulsion.validate()?;
        self.live_mass.validate()?;
        self.hardware.validate()?;
        ensure!(
            self.heat_sample.production.is_finite() && self.heat_sample.dissipation.is_finite(),
            "Invalid thermal sample"
        );
        if let Some(selection) = self.aimed_section {
            selection.validate()?;
        }
        ensure!(
            self.reactor_instability_remaining
                .is_none_or(|remaining| remaining <= 31),
            "Invalid reactor instability window"
        );
        if let Some(entry) = self.building_entry {
            entry.validate()?;
        }
        self.brief.validate()?;
        self.lateral.validate()?;
        ensure!(
            self.chassis() == super::BattleMechChassis::Quad
                || self.lateral == super::BattleLateralState::default(),
            "Only quads can use lateral movement"
        );
        self.validate_hull_down()?;
        ensure!(
            self.position().is_some() == self.map_slot.is_some(),
            "Battlefield placement and slot must coexist"
        );
        ensure!(
            !self.detached || self.position.is_some(),
            "Detached unit lacks retained coordinates"
        );
        self.validate_stealth()?;
        self.validate_null_signature()?;
        for (suite, mode) in [
            (
                super::BattleElectronicSuite::Guardian,
                self.electronics.guardian,
            ),
            (super::BattleElectronicSuite::Angel, self.electronics.angel),
        ] {
            ensure!(
                mode == super::BattleElectronicMode::Off
                    || (self.power == super::BattlePower::Running
                        && self.electronic_suite_available(suite)?),
                "Active electronic suite requires running, available equipment"
            );
        }

        ensure!(
            self.beacons.iter().all(|(section, kinds)| !kinds.is_empty()
                && self
                    .sections
                    .get(section)
                    .is_some_and(|state| state.internal > 0)),
            "Beacon requires a surviving section and nonempty effects"
        );
        self.experience.validate()?;
        self.heat.validate()?;
        if let Some(capacity) = self.reconstructed_cooling {
            ensure!(
                capacity
                    <= self.definition.heat_sinks
                        + super::engine_sink_override::external_capacity(self)?,
                "Invalid reconstructed cooling capacity"
            );
        }
        self.heat_cutoff.validate(
            self.reconstructed_cooling
                .unwrap_or(self.definition.heat_sinks),
        )?;
        ensure!(
            self.hide_elapsed.is_none_or(|elapsed| elapsed <= 100),
            "Invalid hiding timer"
        );
        if let Some(timer) = self.self_destruct {
            timer.validate()?;
        }
        self.validate_radio()?;
        ensure!(
            self.radio_experience_remaining <= 61,
            "Invalid radio experience countdown"
        );
        ensure!(
            self.inferno_remaining <= i32::MAX as u32,
            "Invalid inferno duration"
        );
        self.charge.validate()?;
        ensure!(
            self.limb_recycle.iter().all(|(section, seconds)| matches!(
                section,
                BattleSection::LeftArm
                    | BattleSection::RightArm
                    | BattleSection::LeftLeg
                    | BattleSection::RightLeg
                    | BattleSection::LeftTorso
                    | BattleSection::RightTorso
            ) && (1..=60).contains(seconds)),
            "Invalid physical recovery timer"
        );
        self.searchlight
            .validate(self.definition.has_special("Searchlight"))?;
        self.overheat_clock.validate()?;
        self.stagger.validate()?;
        self.tag.validate()?;
        self.spotter_events.validate()?;
        ensure!(
            self.spotter.is_none_or(|id| id.0 > 0),
            "Invalid spotter identity"
        );
        ensure!(
            self.jump_stabilization <= 12 && (self.jump_stabilization == 0 || !self.is_destroyed()),
            "Invalid jump stabilization countdown"
        );
        ensure!(
            self.ground_elevation
                .is_none_or(|height| self.position.is_some()
                    && height.is_finite()
                    && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&height)
                    && !self.airborne()
                    && ((f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&height)
                        || self.power == super::BattlePower::Off)),
            "Invalid retained terrain altitude"
        );
        self.validate_orbital_drop()?;
        if let Some(event) = self.free_fall {
            ensure!(
                self.flight.is_none()
                    && self.position.is_some()
                    && self.motion.is_some()
                    && (event.grounded()
                        || (self.jump_stabilization == 0
                            && self.stand_timer.is_none()
                            && self.motion.is_some_and(
                                |motion| motion.speed == 0.0 && motion.desired_speed == 0.0
                            ))),
                "Invalid free-falling unit state"
            );
        }
        if let Some(flight) = self.flight {
            ensure!(
                self.jump_stabilization == 0
                    && self.power == super::BattlePower::Running
                    && !self.is_destroyed()
                    && (self.posture == super::BattlePosture::Standing
                        || self.airborne_support_lost())
                    && self.stand_timer.is_none(),
                "Invalid airborne unit state"
            );
            ensure!(
                self.position.is_some()
                    && self.motion.is_some_and(
                        |motion| motion.point == flight.sample().point && motion.speed == 0.0
                    ),
                "Airborne position differs from flight cursor"
            );
        }
        if let Some(timer) = self.stand_timer {
            ensure!(
                self.motion
                    .is_none_or(|motion| motion.speed == 0.0 && motion.desired_speed == 0.0),
                "Stand countdown requires zero travel speed"
            );
            ensure!(
                (1..=60).contains(&timer.remaining()) && !self.is_destroyed(),
                "Invalid stand countdown"
            );
            ensure!(
                matches!(
                    (timer, self.posture),
                    (
                        super::BattleStandTimer::Rising { .. },
                        super::BattlePosture::Standing
                    ) | (
                        super::BattleStandTimer::Recovering { .. },
                        super::BattlePosture::Prone
                    )
                ),
                "Stand countdown disagrees with posture"
            );
        }
        ensure!(
            self.posture != super::BattlePosture::Prone
                || (self.facing == super::BattleFacing::default()
                    && self
                        .motion
                        .is_none_or(|motion| motion.speed == 0.0 && motion.desired_speed == 0.0)),
            "Invalid prone motion or facing"
        );
        ensure!(
            self.target_lock.is_none_or(
                |lock| lock.remaining() <= 8 && self.power == super::BattlePower::Running
            ),
            "Invalid target lock countdown or power state"
        );
        ensure!(self.stun_remaining <= 10, "Invalid crew stun countdown");
        super::crew_recovery::validate(&self.crew_recovery, self.pilot(), self.pilot_injuries())?;
        ensure!(
            self.pilot_injuries <= i8::MAX as u8,
            "Invalid tactical pilot injury count"
        );
        if let super::BattlePower::Starting { remaining } = self.power {
            ensure!((1..=30).contains(&remaining), "Invalid startup countdown");
        }
        ensure!(
            self.power == super::BattlePower::Off || self.position.is_some(),
            "Powered unit must be on a battlefield"
        );
        ensure!(
            !self.hex_sync_pending || self.motion.is_some(),
            "Pending hex update requires motion state"
        );
        if let Some(motion) = self.motion {
            motion
                .propelled(self.power)?
                .validate(self.motion_speed_limit(
                    self.definition.max_speed.max(self.mobility().maximum_speed),
                ))?;
            let position = self
                .position
                .ok_or_else(|| anyhow::anyhow!("Motion requires placement"))?;
            let hex = motion.point.containing_hex()?;
            let agrees = hex.x == i32::from(position.x) && hex.y == i32::from(position.y);
            ensure!(
                if self.hex_sync_pending {
                    !agrees && motion.speed == 0.0
                } else {
                    agrees
                },
                "Motion and hex position disagree"
            );
            ensure!(
                self.power == super::BattlePower::Running
                    || (motion.propelled(self.power)?.speed == 0.0 && motion.desired_speed == 0.0),
                "Unpowered unit cannot propel itself"
            );
        }
        if !self.contract_loadout {
            validate_definition(&self.definition)?;
        }
        self.critical_conditions.validate(
            (self.gyro() == super::BattleGyro::Hardened)
                .then(|| self.system_hits(BattleSystem::Gyro)),
        )?;
        if !self.contract_loadout {
            self.jump_capacity(50)?;
            if self
                .definition
                .attributes
                .get("cargo_space")
                .is_some_and(|value| value != "0")
            {
                self.mass()?;
            }
        }
        ensure!(
            !self.facing.arms_flipped || self.definition.has_special("FlipArms"),
            "Chassis cannot have flipped arms"
        );
        let loadout = self.loadout()?;
        self.tics.validate(loadout.weapons.len())?;
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
            self.fire_modes.iter().all(|(index, mode)| {
                *mode != super::BattleFireMode::Normal
                    && loadout
                        .weapons
                        .get(*index)
                        .is_some_and(|mount| self.contract_loadout || mode.supports(mount.weapon))
            }),
            "Invalid weapon firing mode"
        );
        ensure!(
            self.ammunition_modes.iter().all(|(index, mode)| loadout
                .weapons
                .get(*index)
                .is_some_and(|mount| *mode != super::BattleAmmunitionMode::Normal
                    && (self.contract_loadout || mode.supports(mount.weapon)))),
            "Invalid weapon ammunition mode"
        );
        ensure!(
            self.spent_launchers.iter().all(|index| loadout
                .weapons
                .get(*index)
                .is_some_and(|mount| self.contract_loadout || mount.one_shot)),
            "Invalid spent launcher"
        );
        ensure!(
            self.jammed_weapons.iter().all(|index| loadout
                .weapons
                .get(*index)
                .is_some_and(|mount| mount.weapon.profile().ammunition_per_ton > 0)),
            "Invalid jammed weapon"
        );
        ensure!(
            self.unjam
                .is_none_or(|attempt| (1..=60).contains(&attempt.remaining)
                    && loadout
                        .weapons
                        .get(attempt.weapon_index)
                        .is_some_and(|mount| mount.weapon.profile().ammunition_per_ton > 0)),
            "Invalid unjam attempt"
        );
        super::dumping::validate(self)?;
        ensure!(self.c3_network != Some(0), "Invalid C3 network identity");
        ensure!(self.c3i_network != Some(0), "Invalid C3i network identity");
        self.masc.validate()?;
        self.supercharger.validate()?;
        self.validate_weapon_damage()?;
        super::weapon_power::validate(&self.powered_down_weapons, &loadout.weapons)?;
        super::component_failure::validate(&self.component_failures, |location| {
            loadout.systems.iter().any(|part| part.location == location)
                && !super::damage_field::placeholder(
                    &self.definition().sections[&location.section].criticals[&location.slot]
                        .equipment,
                )
        })?;
        super::weapon_failure::validate(&self.weapon_failures, loadout.weapons.len(), |index| {
            loadout.weapons[index]
                .criticals
                .iter()
                .all(|location| !self.critical_unavailable(*location))
        })?;
        ensure!(
            self.weapon_recycle.iter().all(|(index, remaining)| loadout
                .weapons
                .get(*index)
                .is_some_and(
                    |_| *remaining > 0 && *remaining <= super::weapon_settings::MAX_RECYCLE_SECONDS
                )),
            "Invalid weapon recycle timer"
        );
        ensure!(
            self.breached_sections.iter().all(|section| self
                .sections
                .get(section)
                .is_some_and(|state| state.internal > 0)
                && !self.flooded_sections.contains(section)),
            "Invalid breached Mech section"
        );
        ensure!(
            loadout
                .ammunition
                .iter()
                .enumerate()
                .all(
                    |(index, bin)| !self.breached_sections.contains(&bin.location.section)
                        || self.ammunition[index] == 0
                ),
            "Breached Mech section retains ammunition"
        );
        ensure!(
            self.lost_criticals.iter().all(|location| self
                .definition
                .sections
                .get(&location.section)
                .is_some_and(|section| section.criticals.contains_key(&location.slot))),
            "Lost critical is not installed"
        );
        if self.definition.has_special("ImprovedJJ_Tech") {
            for group in loadout.jump_jet_groups(true)? {
                let lost = group
                    .iter()
                    .filter(|location| self.lost_criticals.contains(location))
                    .count();
                ensure!(
                    lost == 0 || lost == group.len(),
                    "Incomplete improved jump jet loss"
                );
            }
        }
        if self.definition.has_double_heat_sinks() {
            for group in loadout.heat_sink_groups(self.definition.heat_sink_slots())? {
                let lost = group
                    .iter()
                    .filter(|location| self.lost_criticals.contains(location))
                    .count();
                ensure!(
                    lost == 0 || lost == group.len(),
                    "Incomplete double heat sink loss"
                );
            }
        }
        ensure!(
            self.power == super::BattlePower::Off || !self.is_destroyed(),
            "Destroyed unit cannot remain powered"
        );

        ensure!(self.sections.len() == 8, "Unit must have eight sections");
        for (section, original) in &self.definition.sections {
            let state = self
                .sections
                .get(section)
                .ok_or_else(|| anyhow::anyhow!("Missing unit section"))?;
            ensure!(
                self.contract_loadout
                    || state.internal != 0
                    || (state.armor == 0 && state.rear == 0),
                "Destroyed section retains armor"
            );
            ensure!(
                self.contract_loadout
                    || (state.armor <= original.armor
                        && state.internal <= original.internal
                        && state.rear <= original.rear),
                "Unit protection exceeds its definition"
            );
        }
        ensure!(
            self.ammunition.len() == loadout.ammunition.len(),
            "Unit ammunition layout mismatch"
        );
        ensure!(
            self.ammunition
                .iter()
                .zip(&loadout.ammunition)
                .all(|(&rounds, bin)| !self.critical_unavailable(bin.location) || rounds == 0),
            "Unavailable ammunition bin retains rounds"
        );
        ensure!(
            self.ammunition
                .iter()
                .zip(&loadout.ammunition)
                .all(|(&rounds, bin)| rounds <= bin.capacity),
            "Unit ammunition exceeds capacity"
        );
        ensure!(
            !self.hex_sync_pending || self.motion.is_some(),
            "Pending hex update requires motion state"
        );
        if let Some(motion) = self.motion {
            motion
                .propelled(self.power)?
                .validate(self.motion_speed_limit(self.mobility().maximum_speed))?;
        }
        Ok(())
    }

    /// Identity projection shared by inspection and deferred saved-unit readers.
    pub(crate) fn identity(&self) -> StoredBattleUnit {
        StoredBattleUnit {
            name: self.definition.name.clone(),
            template: self.definition.reference.clone(),
            class_code: 0,
            movement_code: match self.chassis() {
                super::BattleMechChassis::Biped => 0,
                super::BattleMechChassis::Quad => 8,
            },
            tons: i64::from(self.definition.tons),
            map: self.position().map(|position| position.map),
        }
    }
}

pub(super) fn replace_critical_modes(
    modes: &mut Vec<String>,
    fire: Vec<String>,
    ammunition: Vec<String>,
) {
    const ALL: &[&str] = &[
        "Destroyed",
        "Disabled",
        "Broken",
        "Damaged",
        "OnTC",
        "RearMount",
        "Hotload",
        "Halfton",
        "OneShot",
        "OneShot_Used",
        "UltraMode",
        "RapidFire",
        "Gattling",
        "Rotary_TwoShot",
        "Rotary_FourShot",
        "Rotary_SixShot",
        "Heat",
        "BackPack",
        "Jettisoned",
        "OmniBase",
        "RocketFired",
        "LBX/Cluster",
        "Artemis/Mine",
        "Narc/Smoke",
        "Cluster",
        "Mine",
        "Smoke",
        "Inferno",
        "Swarm",
        "Swarm1",
        "iNarc_Explosive",
        "iNarc_Haywire",
        "iNarc_ECM",
        "iNarc_Nemesis",
        "AP",
        "Flechette",
        "Incendiary",
        "Precision",
        "Stinger",
        "Caseless",
        "Sguided",
        "ExtendedRange",
        "HighExplosive",
        "MML_LRM",
        "Torpedo",
        "ThunderAug",
        "ThunderVibra",
        "ThunderActive",
    ];
    modes.retain(|mode| !ALL.contains(&mode.as_str()));
    modes.extend(fire);
    modes.extend(ammunition);
}

pub(super) fn remap_set(
    remap: &[Option<usize>],
    values: &std::collections::BTreeSet<usize>,
) -> std::collections::BTreeSet<usize> {
    remap
        .iter()
        .enumerate()
        .filter_map(|(new, old)| old.filter(|old| values.contains(old)).map(|_| new))
        .collect()
}

pub(super) fn remap_map<T: Clone>(
    remap: &[Option<usize>],
    values: &BTreeMap<usize, T>,
) -> BTreeMap<usize, T> {
    remap
        .iter()
        .enumerate()
        .filter_map(|(new, old)| old.and_then(|old| values.get(&old).cloned().map(|v| (new, v))))
        .collect()
}

/// Gate the initial conventional chassis features without silently discarding unknown fields.
fn validate_definition(definition: &BattleTemplate) -> Result<()> {
    super::unit_identity::validate_metadata(&definition.attributes)?;
    super::engine_sink_override::read(&definition.attributes)?;
    super::template_speed::read(&definition.attributes, definition.max_speed)?;
    ensure!(
        (20..=100).contains(&definition.tons) && definition.tons.is_multiple_of(5),
        "Unsupported biped tonnage"
    );
    ensure!(
        definition.max_speed.is_finite()
            && definition.max_speed > 0.0
            && definition.max_speed <= 300.0,
        "Invalid maximum speed"
    );
    ensure!(
        definition.jump_speed.is_finite()
            && definition.jump_speed >= 0.0
            && definition.jump_speed <= definition.max_speed,
        "Invalid jump speed"
    );
    ensure!(
        (10..=100).contains(&definition.heat_sinks),
        "Invalid heat sink count"
    );
    ensure!(
        !definition.name.is_empty()
            && definition.name.len() <= 128
            && !definition.reference.is_empty()
            && definition.reference.len() <= 128,
        "Invalid unit identity"
    );
    super::radio::validate_attributes(&definition.attributes)?;
    for (field, value) in &definition.attributes {
        match field.as_str() {
            "name" | "reference" | "tons" | "max_speed" | "jump_speed" | "heat_sinks"
            | "comment" | "hsengoverride" | "template_speed" => {}
            "type" => ensure!(value.eq_ignore_ascii_case("Mech"), "Unsupported unit type"),
            "move_type" => ensure!(
                value.eq_ignore_ascii_case("Biped") || value.eq_ignore_ascii_case("Quad"),
                "Unsupported movement type"
            ),
            "tac_range" | "lrs_range" | "scan_range" => ensure!(
                value.parse::<u8>().is_ok_and(|range| range <= 127),
                "Invalid sensor range {field}"
            ),
            "unit_era"
            | "unit_tro"
            | "radio_range"
            | "radiotype"
            | "radio"
            | "administrative_tonnage"
            | "administrative_unit_type"
            | "administrative_movement_type"
            | "carrier_maximum_tonnage" => {}
            "computer" => {
                ensure!(value.parse::<u8>().is_ok(), "Invalid electronics rating")
            }
            "cargo_space" => ensure!(value.parse::<u32>().is_ok(), "Invalid cargo space"),
            "max_suits" => ensure!(
                value.parse::<u16>() == Ok(0),
                "Suit capacity is unsupported"
            ),
            "specials" => ensure!(
                value == "-"
                    || (!value.trim().is_empty()
                        && value.split_ascii_whitespace().all(|flag| flag
                            .eq_ignore_ascii_case("FlipArms")
                            || flag.eq_ignore_ascii_case("Searchlight")
                            || flag.eq_ignore_ascii_case("Camo_Tech")
                            || flag.eq_ignore_ascii_case("CritProof_Tech")
                            || flag.eq_ignore_ascii_case("CargoTech")
                            || flag.eq_ignore_ascii_case("SalvageTech")
                            || flag.eq_ignore_ascii_case("Carrier_Tech")
                            || flag.eq_ignore_ascii_case("TripleMyomerTech")
                            || flag.eq_ignore_ascii_case("Masc")
                            || flag.eq_ignore_ascii_case("C3MasterTech")
                            || flag.eq_ignore_ascii_case("C3SlaveTech")
                            || flag.eq_ignore_ascii_case("C3I_Tech")
                            || flag.eq_ignore_ascii_case("SuperCharger_Tech")
                            || flag.eq_ignore_ascii_case("Clan")
                            || flag.eq_ignore_ascii_case("IS_AMS")
                            || flag.eq_ignore_ascii_case("CL_AMS")
                            || flag.eq_ignore_ascii_case("DoubleHS")
                            || flag.eq_ignore_ascii_case("LaserHS_Tech")
                            || flag.eq_ignore_ascii_case("ImprovedJJ_Tech")
                            || flag.eq_ignore_ascii_case("HDGYRO")
                            || flag.eq_ignore_ascii_case("XLGYRO")
                            || flag.eq_ignore_ascii_case("CGYRO")
                            || flag.eq_ignore_ascii_case("SMCPIT")
                            || flag.eq_ignore_ascii_case("ArtemisIV")
                            || flag.eq_ignore_ascii_case("ECM")
                            || flag.eq_ignore_ascii_case("AngelECM_Tech")
                            || flag.eq_ignore_ascii_case("StealthArmor_Tech")
                            || flag.eq_ignore_ascii_case("NullSigSys_Tech")
                            || flag.eq_ignore_ascii_case("AntiAircraft")
                            || flag.eq_ignore_ascii_case("TAG_Tech")
                            || flag.eq_ignore_ascii_case("BeagleProbe")
                            || flag.eq_ignore_ascii_case("LightBAP")
                            || flag.eq_ignore_ascii_case("BloodhoundProbe_Tech")
                            || flag.eq_ignore_ascii_case("FerroFibrous_Tech")
                            || flag.eq_ignore_ascii_case("EndoSteel_Tech")
                            || flag.eq_ignore_ascii_case("HvyFerroFibrous_Tech")
                            || flag.eq_ignore_ascii_case("LtFerroFibrous_Tech")
                            || flag.eq_ignore_ascii_case("XLEngine_Tech")
                            || flag.eq_ignore_ascii_case("LightEngine_Tech")
                            || flag.eq_ignore_ascii_case("XXL_Tech")
                            || flag.eq_ignore_ascii_case("CompactEngine_Tech")
                            || super::BattleTechnology::recognizes(flag)
                            || (0..=56).any(|code| {
                                super::admin_contract::administrative_technology(code)
                                    .is_some_and(|(name, _)| flag.eq_ignore_ascii_case(name))
                            }))),
                "Unsupported chassis specials {value}"
            ),
            _ => anyhow::bail!("Unsupported chassis field {field}"),
        }
    }
    let chassis = definition.chassis()?;
    ensure!(definition.sections.len() == 8, "Missing biped sections");
    for (&section, layout) in &definition.sections {
        let slots = chassis.critical_slots(section);
        ensure!(
            layout.internal > 0
                && layout.internal <= 100
                && layout.armor <= 200
                && layout.rear <= 100,
            "Invalid section protection"
        );
        ensure!(
            layout.criticals.keys().all(|&slot| slot < slots),
            "Critical outside section capacity"
        );
        ensure!(
            layout
                .configuration
                .as_deref()
                .is_none_or(|value| value == "-" || value.eq_ignore_ascii_case("Case")),
            "Unsupported section configuration"
        );
        ensure!(
            matches!(
                section,
                BattleSection::LeftTorso | BattleSection::RightTorso | BattleSection::CenterTorso
            ) || layout.rear == 0,
            "Rear armor outside torso"
        );
    }
    let loadout = BattleLoadout::resolve(definition)?;
    super::BattleEngine::resolve(&loadout, definition.has_special("Clan"))?;
    ensure!(
        ["HDGYRO", "XLGYRO", "CGYRO"]
            .iter()
            .filter(|flag| definition.has_special(flag))
            .count()
            <= 1,
        "Conflicting gyro technologies"
    );
    for (system, count) in [
        (
            BattleSystem::Gyro,
            super::BattleGyro::from_definition(definition).critical_slots(),
        ),
        (BattleSystem::Cockpit, 1),
        (BattleSystem::Sensors, 2),
        (BattleSystem::LifeSupport, 2),
    ] {
        ensure!(
            loadout
                .systems
                .iter()
                .filter(|critical| critical.system == system)
                .count()
                == count,
            "Incomplete conventional {:?} system",
            system
        );
    }
    if definition.has_special("ImprovedJJ_Tech") {
        let jets = loadout.jump_jet_groups(true)?.len();
        ensure!(
            (definition.jump_speed - jets as f64 * 10.75).abs() < 0.001,
            "Improved jump jet capacity does not match installed groups"
        );
    }
    if definition.has_double_heat_sinks() {
        ensure!(
            definition.heat_sinks.is_multiple_of(2),
            "Invalid double heat sink capacity"
        );
    }
    let sinks = loadout
        .systems
        .iter()
        .filter(|critical| critical.system == BattleSystem::HeatSink)
        .count();
    ensure!(
        if definition.has_double_heat_sinks() {
            sinks / definition.heat_sink_slots() * 2
        } else {
            sinks
        } <= usize::from(definition.heat_sinks),
        "More heat sink criticals than declared heat sinks"
    );
    Ok(())
}
