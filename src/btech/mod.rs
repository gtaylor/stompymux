//! BattleTech domain models, asset codecs, and command/notification adapters.
pub mod autopilot;
pub(crate) mod map_lifecycle;
mod special_commands;
pub(crate) mod special_dispatch;
mod special_help;
mod special_registration;
pub(crate) use special_registration::unregister_special;
pub(crate) mod unit_lifecycle;
pub use special_commands::{BattleCommandClass, BattleSpecialCommand, BattleSpecialType};
pub(crate) mod electronics;
pub use electronics::{
    BattleElectronicSuite, BattleElectronics, electronic_field, electronic_fields_pending,
    refresh_electronic_fields, toggle_electronics,
};
mod electronic_field;
pub use electronic_field::{
    BattleElectronicField, BattleElectronicMode, BattleElectronicSource, resolve_electronic_field,
};
mod ammunition_feed;
mod artemis;
mod artillery;
mod artillery_adjustment;
mod artillery_feedback;
mod artillery_firing;
pub use artillery_firing::BattleArtilleryLaunchReport;
mod coordinate_launch;
pub use coordinate_launch::BattleLaunchMisload;
mod artillery_aim;
pub use artillery_aim::{
    BattleArtilleryAim, BattleArtilleryAimInput, BattleArtilleryObserver, BattleArtilleryRange,
    unit_artillery_gunnery_target,
};
mod artillery_impact;
mod artillery_queue;
pub use artillery::{
    BattleArtilleryCell, BattleArtilleryEffect, BattleArtilleryFlight,
    BattleArtilleryImpactPattern, BattleArtilleryMode,
};
pub use artillery_impact::{
    BattleArtilleryHit, BattleArtilleryImpactReport, advance_artillery_flight,
};
pub use artillery_queue::{
    BattleArtilleryShot, advance_artillery_action, artillery_pending, enqueue_artillery,
};
pub use autopilot::{
    AutopilotConfig, AutopilotConfigPatch, AutopilotController, AutopilotFeedback,
    AutopilotFeedbackEvent, AutopilotFeedbackPage, AutopilotFireMode, AutopilotLastSighting,
    AutopilotOrder, AutopilotOrderProgress, AutopilotOrderRecord, AutopilotOrderState,
    AutopilotRangeBand, AutopilotReason, AutopilotState, AutopilotSubmissionMode, LastSighting,
    MAX_FEEDBACK,
};
pub use evacuation::advance_artillery_flight_action;

pub use ammunition_feed::BattleAmmunitionDraw;
pub(crate) mod pods;
pub(crate) mod vehicle_pods;
pub use pods::{
    BattlePodRemoval, BattlePodRow, inspect_pods, pod_status, remove_pod, remove_pod_action,
};
pub use vehicle_pods::{begin_pod_removal, begin_pod_removal_action};
pub(crate) mod inarc;
pub use inarc::set_inarc_ammunition;
pub(crate) mod narc;
mod vehicle_narc;
pub use narc::{
    BattleBeaconKind, BattleNarcReport, BattleUnitSection, toggle_explosive, toggle_narc,
};
pub(crate) mod ams;
mod ams_unit;
pub use ams::{BattleAmsReport, set_ams};
pub(crate) mod unjam;
pub use unjam::{BattleUnjam, advance_unjamming, advance_unjamming_action, begin_unjam};
mod weapon_jam;
pub use artemis::BattleArtemisController;
mod gyro;
pub use gyro::BattleGyro;
mod engine;
pub use engine::BattleEngine;
mod mass;
pub use mass::BattleMass;
mod stacking;
pub use stacking::{
    BattleStackingEntry, BattleStackingInput, BattleStackingRules, resolve_stacking,
};
mod preferences;
mod water_movement;
pub use preferences::set_auto_fall;
mod aim;
mod arcs;
mod assets;
pub use aim::{
    BattleAimModifiers, BattleAimRules, BattleIndirectAim, BattleRangeBracket, BattleSensorAim,
    BattleWeaponRange, aim_modifiers, pilot_aim_modifiers, unit_target_movement_modifier,
};
pub use arcs::{
    BattleContactArc, BattleFacing, BattleTorso, flip_arms, rotate_torso, weapon_bears_on,
};
mod surface_break;
pub use surface_break::{BattleSurfaceBreak, break_bridge, break_ice};
mod balance;
pub use balance::{BattleBalanceCause, BattleBalanceReport};
mod character;
pub(crate) mod character_clear;
pub(crate) mod character_list;
mod character_names;
pub(crate) mod character_show;
pub use character_clear::clear_character;
pub(crate) mod commands;
pub(crate) mod firing;
pub use character::{
    BattleCharacter, BattleCharacterInjury, BattleConsciousnessCheck, injure_character,
    set_character,
};
mod crew;
mod critical;
pub use critical::{BattleAmmunitionHazard, BattleCriticalLoss, destroy_unit_critical};
mod damage;
mod dice;
pub use damage::{BattleDamagePhase, BattleDamageResult, apply_damage_phase};
pub use dice::{BattleDice, roll_unit_dice};
mod equipment;
pub(crate) mod fire_mode;
pub use fire_mode::{BattleFireMode, toggle_flamer_heat, toggle_hotload};
mod geometry;
mod heat;
mod overheat;
pub use heat::{BattleHeat, BattleHeatRates, advance_heat};
pub use overheat::{
    BattleHeatCheck, BattleOverheatClock, BattleOverheatReport, BattleOverheatRules,
    advance_overheat,
};
mod free_fall;
mod hit;
mod impact;
mod jump;
mod jump_fields;
mod jump_flight;
pub use free_fall::{BattleFreeFall, BattleFreeFallStep};
pub(crate) mod jumping;
pub use hit::{BattleHit, BattleHitArc, BattleHitRules, BattleHitTable};
pub use impact::{
    BattleCrewCasualty, BattleImpactEffect, BattleImpactReport, explode_ammunition, resolve_impact,
};
pub use jump::{BattleJumpCapacity, BattleJumpPath, BattleJumpSample};
pub use jump_flight::{BattleJumpFlight, BattleJumpOutcome, BattleJumpStep};
pub use jumping::{advance_jumps, launch_jump};
pub(crate) mod landing;
pub use landing::land_jump;
mod ground_proposal;
mod loadout;
mod loadout_context;
mod map;
mod mobility;
mod motion;
pub use ground_proposal::{
    BattleGroundMotionProposal, propose_mech_ground_motion, propose_vehicle_ground_motion,
};
mod movement_report;
mod template_ammunition;
mod template_check;
pub use mobility::BattleMobility;
pub use template_check::{BattleAmmunitionAdjustment, BattleTemplateCheck, check_template};
mod pilot_health;
mod pilot_injury;
mod placement;
pub use pilot_injury::{
    BattlePilotInjury, BattleTacticalImpact, injure_tactical_pilot, resolve_tactical_impact,
};
mod power;
mod recovery;
mod recovery_output;
pub use recovery::{
    BattleCharacterNotice, BattleRecovery, BattleRecoveryMode, advance_recovery,
    check_character_consciousness, prepare_recovery,
};
mod readiness;
mod weapon_admission;
pub use readiness::{BattleWeaponReadiness, BattleWeaponUse, advance_recycle, spend_weapon};
mod salvo;
mod state;
mod stun;
pub use salvo::{BattleSalvoGroup, BattleSalvoReport, resolve_salvo, resolve_tactical_salvo};
pub use stun::{advance_stun, stun_unit};
mod template;
mod unit;

pub use assets::{read_map, read_template, read_unit_template, read_vehicle_template};
mod unit_template;
pub use map::{BattleHex, BattleMapAsset, Terrain};
pub use state::{
    BtechState, StoredBattleMap, StoredBattleUnit, create_map, create_unit,
    register_empty_battle_unit, reload_map, set_map_optical_sensor, set_map_visibility,
};
pub use unit_template::BattleUnitTemplate;
mod vehicle;
mod vehicle_driving;
mod vehicle_motion;
pub use vehicle_motion::BattleVehicleMotionRules;
mod vehicle_placement;
mod vehicle_power;
pub use vehicle::{BattleVehicle, create_vehicle, damage_vehicle_motive, damage_vehicle_phase};
mod vehicle_mass;
pub use vehicle_mass::BattleVehicleMass;
mod vehicle_engine;
mod vehicle_loadout;
pub use vehicle_engine::{BattleVehicleEngine, BattleVehiclePowerplant};
mod vehicle_template;
pub use template::{BattleSection, BattleTemplate, CriticalDefinition, SectionDefinition};
pub use vehicle_loadout::{BattleVehicleLoadout, VehicleCriticalLocation};
pub use vehicle_template::{BattleVehicleMovement, BattleVehicleSection, BattleVehicleTemplate};

pub use equipment::{BattleSystem, BattleWaterRanges, BattleWeapon, WeaponProfile};
pub use loadout::{AmmunitionBin, BattleLoadout, CriticalLocation, SystemCritical, WeaponMount};

pub use placement::{place_unit, remove_unit};
pub use unit::{BattlePosition, BattleSectionState, BattleUnit};

pub(crate) use crew::player_moved;
pub use crew::{assign_pilot, release_pilot};

pub use power::{BattleNotice, BattlePower, advance_units, start_unit, stop_unit};

/// Render a domain notice through ordinary container notification, staging it until commit.
pub(crate) fn notify_unit(scripts: &crate::Scripts, notice: BattleNotice) -> anyhow::Result<()> {
    notify_unit_text(scripts, notice.unit, &notice.text)
}

/// Stage a formatted cockpit message through the same transaction as domain notices.
pub(super) fn notify_unit_text(
    scripts: &crate::Scripts,
    unit: crate::ObjectId,
    text: &str,
) -> anyhow::Result<()> {
    notify_message(scripts, BattleMessageTarget::Unit(unit), text)
}

/// Stage a typed recipient without accidentally expanding private pilot details to cockpit contents.
pub(crate) fn notify_message(
    scripts: &crate::Scripts,
    recipient: BattleMessageTarget,
    text: &str,
) -> anyhow::Result<()> {
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let world = scripts.world.borrow();
        let (targets, policy) = match recipient {
            BattleMessageTarget::Unit(id) => {
                let mut targets = vec![id];
                targets.extend(cockpit_links::audiences(&world, id));
                (targets, crate::notification::Policy::ROOM)
            }
            BattleMessageTarget::Player(id) => (vec![id], crate::notification::Policy::DIRECT),
        };
        for target in targets {
            crate::notification::send(
                &world,
                &scripts.outbox,
                &crate::lua::configuration(&scripts.lua),
                crate::notification::Request {
                    target,
                    sender: target,
                    document: text.into(),
                    policy,
                    exclusions: None,
                },
            )?;
        }
        Ok(())
    })();
    if result.is_err() {
        scripts.effects.restore(checkpoint);
    }
    result
}

pub use geometry::{BattleHexCoordinate, BattlePoint, BattleRange, unit_elevation, unit_range};

pub(crate) use motion::set_speed_autopilot;
pub use motion::{BattleMotion, BattleMovementRules, advance_motion, set_heading, set_speed};

/// Stage a player-owned condition notice through the normal notification boundary.
pub(crate) fn notify_character(
    scripts: &crate::Scripts,
    notice: BattleCharacterNotice,
) -> anyhow::Result<()> {
    recovery_output::publish(scripts, notice)
}

mod los;
mod los_trace;
pub use los::{BattleTerrainLos, ground_terrain_los, unit_terrain_los};

mod sensors;
pub use sensors::{
    BattleLight, BattleSensorConditions, BattleSensorMode, BattleSensorReport, map_optical_contact,
    optical_contact,
};

mod detection;
pub use detection::{
    BattleDetection, BattleDetectionRules, BattleScanTarget, BattleSensorArc, BattleSensorAttempt,
    BattleSensorScan, BattleSensorScanReport, roll_optical_detection, scan_optical_target,
};

mod sensor_selection;
pub use sensor_selection::{
    BattleSensorChange, BattleSensorPair, BattleSensorSelection, advance_sensor_selection,
    select_optical_sensors,
};

mod contacts;
pub(crate) use contacts::contact_facts;
pub use contacts::{
    BattleContact, BattleContactRules, BattleContactSensors, BattleContactTransition,
    BattleContactUpdate, BattleContactView, update_optical_contact, visible_contact,
    visible_contacts,
};

mod skills;
pub use skills::{
    BattleCharacterValue, BattleSkillCategory, gunnery_target, perception_target,
    set_character_value, unit_gunnery_target, unit_piloting_target,
};

mod scanner;
pub(crate) use scanner::notify_contact;
pub use scanner::{
    BattleContactEvent, BattleSensorSignature, optical_scanner_observers, refresh_optical_scanners,
    set_sensor_signature,
};

mod targeting;
pub use targeting::{BattleTargetLock, advance_target_locks, select_target};

pub(crate) mod fire_target;
pub use fire_target::BattleFireTarget;
mod shot;
pub use shot::{
    BattleGlancingMode, BattleRecoilReport, BattleShotReport, BattleShotRules, resolve_shot,
};

mod piloting;
mod vehicle_arcs;
mod vehicle_control_damage;
pub use vehicle_control_damage::{BattleVehicleControlHit, damage_vehicle_controls};
mod vehicle_critical_table;
pub use vehicle_critical_table::{
    BattleVehicleCriticalEffect, BattleVehicleCriticalReport, BattleVehicleCriticalRules,
    BattleVehicleCriticalTable, roll_vehicle_critical,
};
mod vehicle_critical;
pub use vehicle_critical::{destroy_vehicle_critical, select_vehicle_weapon_critical};
mod vehicle_readiness;
pub use vehicle_readiness::{BattleVehicleWeaponUse, reserve_vehicle_weapon};
mod vehicle_hit;
mod vehicle_piloting;
mod vehicle_turret;
pub use piloting::{BattlePilotNotice, BattlePilotingCheck, roll_piloting};
pub use vehicle_hit::{
    BattleVehicleFasaHitRules, BattleVehicleHit, BattleVehicleHitCondition, BattleVehicleMotiveHit,
};
pub use vehicle_turret::{lock_vehicle_turret, set_turret, turret_readout};

mod fall;
pub use fall::{BattleFallReport, BattleFallRules, BattlePosture, resolve_fall};

pub(crate) mod stand;
pub use stand::{
    BattleStandAttempt, BattleStandMode, BattleStandTimer, advance_standing, begin_stand,
    stand_target,
};

mod stagger;
pub use stagger::{
    BattleStagger, BattleStaggerHit, BattleStaggerMode, BattleStaggerReport, BattleStaggerRules,
    advance_stagger,
};

mod flooding;
pub use flooding::flood_unit;
mod section_exposure;
pub use section_exposure::{BattleSectionExposure, BattleSectionExposureReport};

pub(crate) mod ammunition_mode;
pub use ammunition_mode::{BattleAmmunitionMode, toggle_artemis, toggle_cluster, toggle_lbx};

pub(crate) mod ultra;
pub use ultra::toggle_ultra;

pub(crate) mod rapid;
pub use rapid::toggle_rapid;

pub(crate) mod rotary;
pub use rotary::set_rotary;

pub(crate) mod gatling;
pub use gatling::toggle_gatling;

mod weapon_controls;

pub(crate) mod precision;
pub use precision::toggle_precision;

pub(crate) mod flechette;
pub use flechette::toggle_flechette;

pub(crate) mod armor_piercing;
pub use armor_piercing::toggle_armor_piercing;

pub(crate) mod caseless;
pub use caseless::toggle_caseless;

pub(crate) mod incendiary;
pub use incendiary::toggle_incendiary;

mod broadcast;
pub use broadcast::{BattleMessageTarget, observer_messages};

mod searchlight;
pub use searchlight::{
    BattleSearchlight, advance_searchlights, toggle_searchlight, unit_illuminated,
};

pub use preferences::set_searchlight_warning;
pub use searchlight::{illumination_pending, refresh_illumination};

mod combat_warnings;
pub use preferences::{set_ammunition_warning, set_armor_warning};

pub use preferences::set_friendly_fire_safety;

pub(crate) mod status;
pub use firing::weapon_status;
pub use status::unit_status;

mod status_export;

pub(crate) mod physical;
pub use physical::{
    BattleArm, BattleArmAttackReport, BattleArmRejection, BattleArmSelection, BattleLeg,
    BattlePhysicalAttack, BattlePhysicalProfile, BattlePhysicalReport, BattlePhysicalRules,
    kick_profile, punch_profile, resolve_kick, resolve_punch, resolve_trip, trip_profile,
};

mod myomer;

pub use physical::{BattleArmAttack, arm_attack_profile, resolve_arm_attack};

mod club;
pub use club::grab_club;
pub use physical::{club_profile, resolve_club};

mod charge;
pub use charge::{
    BattleChargeBalance, BattleChargeProfile, BattleChargeReport, BattleChargeRules,
    charge_profile, resolve_charge,
};

pub use charge::{BattleMutualChargeAttempt, BattleMutualChargeReport, resolve_mutual_charge};

mod charge_tracking;
pub use charge_tracking::{
    BattleChargePolicy, BattleChargeSelection, BattleChargeState, select_charge,
};

mod dfa;
pub use dfa::{BattleDfaBalance, BattleDfaProfile, BattleDfaReport, dfa_profile, resolve_dfa};

pub use jumping::launch_dfa;

mod experience;
mod physical_experience;
pub use experience::{BattleExperienceAward, BattleExperienceRules, award_character_experience};

pub(crate) mod skill_catalog;
pub use skill_catalog::{
    BATTLE_SKILLS, BattleSkillDefinition, BattleSkillProgress, award_skill_experience,
    retain_character_experience, set_skill_threshold, skill_definition, skill_progress,
    skill_threshold,
};

mod evacuation;
pub(crate) use evacuation::evacuate;

pub use evacuation::resolve_impact_action;

mod character_pilot;
pub use character_pilot::{
    BattleCharacterPilotInjury, BattleCharacterPilotStatus, injure_character_pilot,
};

pub use evacuation::{
    injure_character_pilot_action, resolve_vehicle_armor_damage_action,
    resolve_vehicle_critical_action,
};

pub use evacuation::flood_unit_action;

pub use evacuation::fall_unit_action;

/// Character-capable upward ice breakout host action.
pub use evacuation::break_ice_upward_action;
pub use evacuation::break_surface_action;

/// Host airborne tick with character casualty publication.
pub use evacuation::advance_jumps_action;

/// Host crowding collision with character casualty publication.
pub use evacuation::stacking_action;

/// Ground movement with atomic character casualty publication.
pub use evacuation::advance_motion_action;

/// Physical attack host action with character casualty publication.
pub use evacuation::physical_attack_action;

pub use evacuation::arm_attack_action;
/// Typed arm sequence and its atomic character-aware host action.
pub use physical::BattleArmAttackChoice;

/// One-way charge host action with character casualty publication.
pub use evacuation::charge_action;

/// Atomic mutual charge host action.
pub use evacuation::mutual_charge_action;

/// Character-aware DFA host action.
pub use evacuation::dfa_action;

pub use evacuation::stand_action;

pub use evacuation::stagger_action;

pub use evacuation::ammunition_explosion_action;

pub use evacuation::overheat_action;

pub use evacuation::salvo_action;

pub use evacuation::shot_action;

mod gunnery_experience;
pub use gunnery_experience::{BattleGunneryExperienceChance, BattleGunneryExperienceInput};

pub use gunnery_experience::{
    BattleGunneryExperienceMode, BattleUnitExperience, gunnery_experience_eligible,
    set_unit_experience,
};

pub use gunnery_experience::{
    BattleGunneryAwardRequest, BattleGunneryExperienceAward, award_classic_gunnery_experience,
};

mod effective_speed;
pub use effective_speed::unit_effective_maximum_speed;

mod battle_value;
pub use battle_value::BattleValue;
pub use battle_value::unit_battle_value;

mod battle_value_experience;
pub use battle_value_experience::{
    BattleValueExperienceAward, BattleValueExperienceCalculation, BattleValueExperienceInput,
    award_battle_value_gunnery_experience, pilot_battle_value_modifier,
};

pub use gunnery_experience::{BattleShotExperienceAward, award_gunnery_experience};

mod channels;
pub use channels::{BattleChannel, BattleChannelMessage};

mod movement_experience;
pub use movement_experience::BattleMovementExperience;

pub use evacuation::land_action;

mod stealth;
pub use stealth::{advance_stealth, toggle_stealth};

mod signature;
pub use signature::{
    BattleSignatureState, BattleSignatureTransition, advance_null_signature, toggle_null_signature,
};

pub use sensors::infrared_heat_modifier;

mod seismic;
pub use seismic::{BattleSeismicRules, BattleSeismicTarget, seismic_contact};

pub use seismic::{BattleSensorSignal, advance_sensor_signals, configure_sensor_policy};

mod electromagnetic;
pub use electromagnetic::{
    BattleElectromagneticRules, BattleElectromagneticTarget, clear_recent_fire,
    electromagnetic_contact,
};

mod radar;
pub use radar::{BattleRadarTarget, radar_contact};

mod active_probe;
pub use active_probe::{BattleActiveProbe, active_probe_contact};

mod tag;
pub use tag::{BattleTagState, advance_tags, select_tag, tagged_by};

pub(crate) mod semiguided;
pub use semiguided::toggle_semiguided;
pub(crate) mod stinger;
pub use stinger::toggle_stinger;

mod spotter;
pub use spotter::{BattleSpotterTarget, select_spotter, spotter_target};

mod hex_visibility;
pub use hex_visibility::{hex_sensor_visibility, hex_visible};

pub use targeting::{BattleHexLock, BattleHexTargetMode, BattleTargetSelection, select_hex_target};

pub use targeting::hex_occupant;

pub(crate) mod map_bits;
mod map_slots;
pub use map_slots::map_unit_order;

mod hex_aim;
pub use hex_aim::{BattleHexAimModifiers, hex_aim_modifiers, pilot_hex_aim_modifiers};

mod launch_roll;
mod weapon_launch;

mod woodland;
pub use woodland::{BattleWoodlandEffect, BattleWoodlandIntent, resolve_woodland_effect};

mod woodland_map;
pub use woodland_map::{BattleWoodlandChange, apply_woodland_clearing};

mod decorations;
pub use decorations::{
    BattleDecoration, BattleDecorationKind, advance_map_smoke, map_smoke_pending,
    set_map_decoration,
};

mod wind;
pub use wind::set_map_wind;

mod fire_spread;
pub use fire_spread::{advance_map_fire, map_fire_pending};

mod woodland_action;
mod woods_absorption;
pub use woodland_action::{BattleWoodlandAttack, BattleWoodlandImpact, resolve_woodland_attack};
pub use woods_absorption::BattleWoodsAbsorption;

mod weapon_groups;

mod hex_shot;
pub use hex_shot::{BattleHexShotReport, resolve_hex_shot};

mod fire_report;
pub use fire_report::BattleFireReport;
mod direct_effects;
mod fire_feedback;
mod hex_firing;
mod launch_feedback;
mod vehicle_firing;

mod surface_weapon;
pub use surface_weapon::BattleSurfaceWeaponImpact;

mod building;
pub use building::{BattleBuildingState, set_building_state};

mod building_entrance;
pub use building_entrance::{BattleBuildingEntrance, set_building_entrance};

mod building_damage;
pub use building_damage::{
    BattleBuildingImpact, advance_building_repairs, building_repair_pending,
};

pub(crate) mod map_view;
pub use map_view::view_map_action;
pub(crate) mod map_mine;
pub use map_mine::{BattleMinePlacement, add_mine_action};
mod minefield;
pub use minefield::{BattleMineKind, BattleMinefield, insert_minefield, set_minefield};

mod mine_activation;
pub use mine_activation::{
    BattleMineActivation, BattleMineResponse, BattleMineTriggerReason, mine_activations,
};

mod blast_damage;
pub use blast_damage::BattleBlastImpact;
mod mine_blast;
pub use evacuation::resolve_mine_blast_action;
pub use mine_blast::{BattleMineBlastHit, BattleMineBlastReport, resolve_mine_blast};

mod inferno;
pub use inferno::{advance_inferno_burns, apply_inferno_burn, extinguish_inferno_in_water};

mod inferno_hit;
pub use inferno_hit::{BattleInfernoHit, resolve_inferno_hit};

pub(crate) mod inferno_ammunition;
pub use inferno_ammunition::toggle_inferno;

mod mine_event;
pub use mine_event::{BattleMineEventReport, activate_mines};

mod command_mines;
pub use command_mines::{BattleCommandMineReport, detonate_command_mines};
pub use evacuation::detonate_command_mines_action;

pub(crate) mod radio;
pub use radio::{
    BattleRadioCapabilities, BattleRadioChannel, BattleRadioMode, set_radio_frequency,
    set_radio_mode, set_radio_title,
};

mod radio_delivery;
pub use radio_delivery::{BattleDigitalRadioReport, BattleRadioReception, resolve_digital_radio};

mod radio_analog;
pub use radio_analog::{BattleAnalogRadioReport, resolve_analog_radio};

pub(crate) mod radio_transmission;
pub use radio_transmission::{BattleRadioDelivery, BattleRadioTransmission, send_radio_action};

mod radio_audit;
pub use radio_audit::set_radio_frequency_action;

mod radio_experience;
pub use radio_experience::award_radio_experience;

mod observer;
pub use observer::{set_observer, unit_observer};

pub(crate) mod radio_targeted;
pub use radio_targeted::{
    BattleTargetedRadioReport, resolve_targeted_radio, send_targeted_radio_action,
};

pub(crate) mod scan;
mod scan_summary;
mod scan_weapons;
pub use scan::{BattleSensorRanges, scan_hex_unit_action, scan_unit, scan_unit_action};

mod scan_building;
pub use scan_building::{BattleBuildingScan, scan_building, scan_building_action};

mod perception_check;
mod scan_mines;
pub use scan_mines::{BattleHexScan, BattleMineScan, scan_hex_action, scan_mines};

mod scan_selected;
pub use scan_selected::{BattleSelectedScan, scan_selected_action};

pub(crate) mod report;
pub use scan::report_unit;

mod view_center;
pub use view_center::{
    BattleViewCenter, BattleViewKind, BattleViewPosition, parse_view_center, resolve_view_center,
};

mod viewport;
pub use viewport::{BattleViewDimensions, BattleViewport, resolve_viewport};

pub(crate) mod long_range_map;
pub use long_range_map::{BattleLongRangeMap, BattleLongRangeMode, long_range_map};

mod map_style;

mod hex_illumination;
pub use hex_illumination::hex_illuminated;

pub(crate) mod tactical_map;
pub use tactical_map::{BattleTacticalMap, tactical_map};

mod landing_zone;
pub use landing_zone::{BattleLandingExclusion, BattleLandingSuitability, set_landing_exclusion};

pub(crate) mod find_center;
pub use find_center::{BattleHexCenterReport, find_center};

pub(crate) mod navigation;
pub use navigation::{BattleNavigationReport, navigate};

pub(crate) mod view_preferences;
pub use view_preferences::{BattlePlayerPreferences, set_view_dimensions, view_dimensions};
mod player_configuration;
pub use player_configuration::{
    BattlePersonalEquipment, BattlePersonalLoadout, BattlePlayerConfiguration,
    player_configuration, set_player_configuration,
};
mod inspection;
pub use inspection::{
    InspectionArmor, InspectionCritical, InspectionPart, InspectionTechnology, InspectionWeapon,
    compose_unit_raw_inspection, compose_vehicle_raw_inspection, inspect_composed_unit_armor,
    inspect_composed_vehicle_armor, inspect_raw_template_armor, inspect_raw_template_battle_value,
    inspect_raw_template_criticals, inspect_raw_template_engine, inspect_raw_template_inventory,
    inspect_raw_template_technologies, inspect_raw_template_weapons, inspect_section_condition,
    inspect_technologies, inspect_template_armor, inspect_template_critical_text,
    inspect_template_criticals, inspect_template_inventory, inspect_template_status_text,
    inspect_template_weapon_text, inspect_template_weapons, inspect_unit_armor,
    inspect_unit_criticals, inspect_unit_inventory, inspect_unit_tic, inspect_unit_weapons,
    inspect_vehicle_armor, inspect_vehicle_criticals, inspect_vehicle_inventory,
    inspect_vehicle_section_condition, inspect_vehicle_technologies,
    inspect_vehicle_template_armor, inspect_vehicle_template_critical_text,
    inspect_vehicle_template_criticals, inspect_vehicle_template_inventory,
    inspect_vehicle_template_status_text, inspect_vehicle_template_weapon_text,
    inspect_vehicle_template_weapons, inspect_vehicle_tic, inspect_vehicle_weapons,
    inspection_battle_value, inspection_effective_maximum_speed, inspection_engine_rating,
    inspection_section, inspection_section_code, inspection_template_battle_value,
    inspection_vehicle_engine_rating, inspection_vehicle_engine_values, inspection_vehicle_section,
    inspection_vehicle_section_code, inspection_vehicle_section_for,
    inspection_vehicle_template_battle_value,
};
mod unit_operations_contract;
pub(crate) use unit_operations_contract::{
    UnitDamageRequest, apply_unit_damage_action, configure_unit_ammunition,
    configure_vehicle_ammunition, install_unit_special, install_unit_weapon_named,
    install_vehicle_special, install_vehicle_weapon_named, load_unit_template,
    reset_unit_criticals, restock_unit_ammunition, restock_vehicle_ammunition,
    set_unit_weapon_modes, unit_piloting_check_action, unit_template_source,
    vehicle_template_source,
};
mod admin_contract;
pub use admin_contract::{
    AdministrativeRepairKind, ReattachHull, administrative_assigned_pilot,
    administrative_is_fixable, administrative_section_info, administrative_section_valid,
    administrative_template_movement, administrative_template_tonnage, administrative_unit_class,
    administrative_unit_movement, administrative_unit_tonnage, apply_administrative_repair,
    clear_administrative_technologies, set_administrative_armor, set_administrative_assigned_pilot,
    set_administrative_cargo, set_administrative_heat_sinks, set_administrative_movement_type,
    set_administrative_radio_quality, set_administrative_scalar, set_administrative_technology,
    set_administrative_unit_type,
};
mod unit_configuration;
pub use unit_configuration::{
    BattleUnitConfiguration, set_unit_configuration, set_unit_identity_configuration,
    unit_configuration,
};
mod template_contract_assets;
pub use template_contract_assets::{
    TemplateRegistryCache, read_resolved_raw_template, read_resolved_template,
    resolve_template_path, resolve_template_path_bytes_cached, resolve_template_path_cached,
    write_template,
};
mod template_cost;
pub use template_cost::{raw_template_base_cost, template_base_cost, vehicle_template_base_cost};
mod event_telemetry;
pub use event_telemetry::BattleEventTelemetry;
mod character_value_contract;
pub use character_value_contract::{
    CharacterValueDefinition, character_raw_value, character_saved_value,
    character_value_definition, character_value_definition_code, character_value_definitions,
    set_character_raw_value, set_character_skill_experience, set_character_skill_target,
};
mod lua_map_contract;
pub use lua_map_contract::{
    BattleMapEmitAudience, BattleMapLos, BattleMapMember, BattleMapSpatialPoint,
    battle_map_hex_los, battle_map_hex_point, battle_map_members, battle_map_spatial_range,
    battle_map_unit_by_label, battle_map_unit_los, battle_map_unit_map, battle_map_unit_point,
    emit_battle_map_trusted_action, load_battle_map_trusted_action, place_battle_map_unit,
    update_battle_map_links_trusted_action,
};

mod contact_preferences;
pub(crate) mod contact_report;
pub use contact_preferences::{
    BattleBuildingContactMode, BattleContactPreferences, contact_preferences, filtered_contacts,
    set_contact_preferences,
};

pub use contact_preferences::{BattleContactOptions, parse_contact_options};

mod building_contacts;
pub use building_contacts::{BattleBuildingContact, building_contacts};

pub(crate) mod brief;
pub use brief::{BattleBriefReport, BattleBriefSettings, brief};

pub use contact_preferences::parse_contact_options_for_display;

mod contact_status;

pub use preferences::set_autocon_shutdown;

pub(crate) mod lateral;
pub use lateral::{BattleLateralMode, BattleLateralState, lateral, set_lateral};

pub(crate) mod turnmode;
pub use turnmode::turnmode;

pub(crate) mod bootlegger;
pub use bootlegger::{BattleBootleggerReport, bootlegger, bootlegger_modifier};

pub(crate) mod eta;
pub use eta::{BattleEtaReport, eta, eta_action};

pub(crate) mod bearing;
pub use bearing::{BattleBearingReport, bearing};

mod navigation_measurement;
pub(crate) mod range_display;
pub use range_display::{BattleRangeReport, range_display};

pub(crate) mod vector_display;
pub use vector_display::{BattleVectorReport, vector_display};

pub(crate) mod motion_controls;
pub use motion_controls::motion_readout;

pub(crate) mod dumping;
pub use dumping::{
    BattleDump, BattleDumpIgnition, BattleDumpSelection, advance_dumping, begin_dump, dump,
};

mod boosters;

pub(crate) mod booster_control;
pub use booster_control::{
    BattleBoosterState, advance_boosters_action, masc, supercharger, toggle_masc,
    toggle_supercharger,
};

mod c3_hardware;
mod network_unit;
pub use c3_hardware::BattleC3Hardware;

pub(crate) mod command_network;

mod network_range;
pub use network_range::BattleNetworkRange;

pub(crate) mod network_message;

pub(crate) mod network_status;

pub(crate) mod network_targets;

mod radio_scanning;
pub use radio_scanning::BattleFrequencyScan;

mod chassis;
pub use chassis::BattleMechChassis;

mod motion_speed;

mod leg_support;

mod weapon_failure;
pub use weapon_failure::{BattleEquipmentFailure, set_weapon_failure};
mod vehicle_weapon_failure;
pub use vehicle_weapon_failure::{BattleVehicleWeaponJam, jam_vehicle_weapon};

pub use vehicle_turret::{begin_vehicle_turret_repair, jam_vehicle_turret};

mod vehicle_critical_resolution;
pub use vehicle_critical_resolution::{BattleVehicleCriticalResolution, resolve_vehicle_critical};

pub use vehicle_weapon_failure::{BattleVehicleMainWeaponJam, jam_vehicle_main_weapon};

mod vehicle_ammunition_cascade;
pub use vehicle_ammunition_cascade::{
    BattleVehicleAmmunitionCascade, discharge_vehicle_ammunition_cascade,
};

mod vehicle_internal_damage;
pub use vehicle_internal_damage::{BattleVehicleInternalDamage, resolve_vehicle_internal_damage};

mod vehicle_explosion;
pub use vehicle_explosion::BattleVehicleExplosion;

mod vehicle_armor_damage;
pub use vehicle_armor_damage::{
    BattleVehicleArmorDamage, BattleVehicleArmorHit, resolve_vehicle_armor_damage,
};

mod vehicle_impact;
pub use vehicle_impact::{BattleVehicleImpact, BattleVehicleImpactRules, resolve_vehicle_impact};

mod vehicle_aim;

mod vehicle_shot;
pub use vehicle_shot::check_vehicle_shot;

mod unjam_unit;
mod vehicle_launch;
pub use vehicle_launch::{BattleVehicleLaunch, BattleVehicleLaunchRequest, launch_vehicle_weapon};

mod vehicle_salvo;
pub use vehicle_salvo::{
    BattleVehicleSalvoGroup, BattleVehicleSalvoReport, BattleVehicleSalvoRequest,
    resolve_vehicle_salvo,
};

mod vehicle_fire;
pub use vehicle_fire::{BattleVehicleShotReport, BattleVehicleShotRules, fire_vehicle_shot};

mod target_salvo;
pub use target_salvo::BattleTargetSalvo;

mod crew_recovery;

pub(crate) mod vehicle_burning;
pub use vehicle_burning::{
    BattleVehicleFireTick, BattleVehicleHeatExposure, BattleVehicleInfernoHit,
    advance_vehicle_fires, begin_vehicle_extinguishing, begin_vehicle_extinguishing_action,
    resolve_vehicle_heat_exposure, resolve_vehicle_inferno_hit,
};

mod vehicle_injuries;
mod vehicle_motive_effects;
pub use evacuation::{
    advance_vehicle_fires_action, resolve_vehicle_fire_exposure_action,
    resolve_vehicle_heat_exposure_action, resolve_vehicle_inferno_hit_action,
};
pub use vehicle_burning::{
    BattleVehicleFireEffects, BattleVehicleFireExposure, resolve_vehicle_fire_exposure,
};

mod fall_profile;
mod vehicle_fall;
pub use evacuation::vehicle_fall_action;
pub use vehicle_fall::{BattleVehicleFallReport, resolve_vehicle_fall};

mod reverse_slope;

mod cliff;
mod vehicle_cliff;

mod vehicle_water;

mod terrain_control;

mod vehicle_obstacle;

mod transport_loss;

mod vtol_hit;
pub use vtol_hit::{BattleRotorHit, BattleVtolHit};

mod rotor_damage;
pub use rotor_damage::BattleRotorDamage;

mod vtol_speed;

mod vtol_fuel;
pub use vtol_fuel::{
    BattleVtolFuel, BattleVtolFuelStatus, BattleVtolFuelUse, set_vtol_fuel, vtol_fuel_status,
};

pub(crate) mod vtol_controls;
mod vtol_flight;
pub use vtol_controls::{begin_vtol_takeoff, set_vtol_vertical_speed, vtol_vertical_readout};
pub use vtol_flight::{BattleVtolFlight, BattleVtolFlightPhase, BattleVtolTakeoff};

mod vtol_landing;
pub use vtol_landing::BattleVtolLanding;

mod vtol_motion;
pub use vtol_motion::{BattleVtolMotionStep, BattleVtolSurfaceContact};

mod vtol_path;
pub use vtol_path::BattleVtolPath;

mod vtol_environment;
mod vtol_updates;
pub use vtol_environment::{BattleVtolEnvironment, advance_vtol_environment};
mod vtol_crash;
pub use vtol_crash::{BattleVehicleDescentEvent, advance_vtol_fall, resolve_vtol_crash};

mod vtol_emergency;

mod map_boundary;
pub use map_boundary::{BattleLinkedMarker, set_linked_marker, set_map_wrapping};

mod building_routes;
pub use building_routes::{
    BattleBuildingEntryPoint, BattleBuildingExit, building_entry_destination,
    building_exit_destination, set_building_entry_point, set_building_exit,
    set_building_return_link,
};

mod map_transfer;
pub use map_transfer::transfer_unit;

mod building_entry;
pub use building_entry::{
    BattleBuildingEntry, advance_building_entry, begin_building_entry, building_entry,
    building_entry_destination_for_unit, building_entry_lock_allows, clear_building_entry,
};

pub(crate) mod building_actions;
pub use building_actions::{
    BattleBuildingArrival, advance_building_entries_action, begin_building_entry_action,
    building_entries_pending, exit_building_action, publish_building_arrivals,
};

mod building_exit;
mod building_step;
pub use building_exit::{building_exit_for_unit, exit_building};

mod towing;
pub use towing::set_tow;

mod load;
pub use load::{BattleUnitLoad, unit_load};

pub use motion_controls::throttle_maximum;

pub(crate) use motion::set_speed_configured;

pub(crate) use status::unit_status_configured;

mod pickup;
pub use pickup::{pickup_admission, set_towable, unit_towable};

pub use pickup::prepare_pickup;

pub use vtol_crash::{advance_vehicle_descent, begin_vehicle_descent};

mod tow_release;
pub use tow_release::release_tow;

mod pickup_transaction;
pub use pickup_transaction::{BattlePickupReport, pickup_unit};

pub(crate) mod tow_actions;
pub use tow_actions::tow_action;

pub(crate) mod dig;
pub use dig::{BattleDigState, dig_action, dig_unit};

pub(crate) mod hull_down;
pub use hull_down::{BattleHullDownState, hull_down_action, set_hull_down};

pub(crate) mod fortification;
pub use fortification::{set_fortified, unit_fortified};

pub use geometry::unit_altitude;

pub use radio::unit_radio_capabilities;

pub(crate) mod tic;
pub use tic::{BattleTicEdit, BattleTics, battle_tic, edit_battle_tic};

pub use tic::{BattleTicShot, fire_battle_tics};

pub(crate) mod heat_cutoff;
pub use heat_cutoff::{BattleHeatCutoff, toggle_battle_heat_cutoff};

pub(crate) mod ammunition_preference;
pub use ammunition_preference::set_battle_ammunition_section;

pub(crate) mod automatic_turret;
pub use automatic_turret::{
    advance_battle_automatic_turrets, battle_automatic_turrets_pending,
    toggle_battle_automatic_turret,
};

mod sensor_flash;
pub use sensor_flash::{
    advance_battle_sensor_flashes, battle_sensor_flashes_pending, battle_unit_blinded,
};

mod reactor_explosion;
pub use reactor_explosion::{
    BattleReactorBlastHit, BattleReactorExplosion, reactor_explosion_action,
};

pub(crate) mod self_destruct;
pub use self_destruct::{
    BattleSelfDestruct, BattleSelfDestructOutcome, advance_battle_self_destructs_action,
    battle_self_destructs_pending, self_destruct_action,
};

pub use self_destruct::set_battle_self_destruct_safe;

pub(crate) mod reactor_instability;
pub use reactor_instability::{
    advance_battle_reactor_windows, battle_reactor_windows_pending, configure_battle_reactor_policy,
};

pub(crate) mod wreck_cleanup;
pub use wreck_cleanup::{advance_battle_wrecks_action, battle_wrecks_pending};

pub(crate) mod prone;
pub use prone::{BattleProneReport, prone_action};

pub(crate) mod hiding;
pub use hiding::{advance_battle_hiding, battle_hiding_pending, begin_battle_hiding};

mod weapons_hold;
pub use weapons_hold::{battle_weapons_hold, set_battle_weapons_hold};

mod visibility;
pub use visibility::{BattleVisibility, battle_visibility, set_battle_visibility};

mod combat_safe;
pub use combat_safe::{battle_combat_safe, set_battle_combat_safe};

pub(crate) mod swarm;
pub use swarm::{BattleSwarmHop, BattleSwarmReport, toggle_swarm};

pub(crate) mod special_rounds;
pub use special_rounds::toggle_missile_rounds;

mod weapon_damage;
pub use weapon_damage::{BattleWeaponDamage, BattleWeaponDamageEffects, BattleWeaponDamageKind};

pub(crate) mod sight;
pub use sight::{BattleSightAim, BattleSightReport};

mod weapon_geometry;

pub(crate) mod weapon_reports;
pub use weapon_reports::{
    BattleEquipmentCondition, BattleWeaponDiagnostic, BattleWeaponSpecification,
    weapon_diagnostic_text, weapon_diagnostics, weapon_specification_text, weapon_specifications,
};

pub(crate) mod critical_report;
mod equipment_display;
pub use critical_report::{
    BattleCriticalInspection, BattleCriticalReport, critical_report, critical_status,
};

pub(crate) mod aimed_target;
pub use aimed_target::{BattleAimSelection, aimed_section, set_aimed_section};

mod aimed_hit;

mod hit_direction;

mod target_hit;

pub(crate) mod weapon_settings;
pub use weapon_settings::{
    BattleWeaponSettings, BattleWeaponValues, set_weapon_battle_value, set_weapon_recycle,
};

mod inventory;
pub use inventory::{
    BattleInventoryEntry, inventory, part_cost, set_inventory_quantity,
    set_inventory_quantity_action, set_part_cost, set_part_store_quantity,
};

mod parts;
mod parts_catalogue;
pub use parts::{BattlePart, BattlePartKind, inventory_mass, set_inventory_named};

mod cargo_bay;
pub use cargo_bay::{
    BattleCargoTransferPoint, check_cargo_transfer_point, set_cargo_transfer_point,
};

pub(crate) mod cargo;
pub use cargo::{
    BattleCargoOperation, BattleCargoRow, cargo_manifest, transfer_cargo, transfer_cargo_action,
};

mod stock_selection;

pub(crate) mod stock_commands;
pub use stock_commands::{BattleInventoryChange, add_stores_action, change_inventory_action};

pub(crate) mod weapon_power;
pub use weapon_power::disable_gauss_weapon;

pub(crate) mod mml;
pub use mml::toggle_mml_ammunition;

pub(crate) mod atm;
pub use atm::toggle_atm_ammunition;

pub(crate) mod inventory_cleanup;
pub use inventory_cleanup::{BattleInventoryCleanup, clean_inventory, clean_inventory_action};

mod clouds;
pub(crate) mod map_environment;
pub use clouds::set_map_cloud_base;
mod vacuum;
pub use map_environment::{BattleMapEnvironment, set_map_environment, set_map_environment_action};

pub(crate) mod map_ice;
pub use map_ice::{BattleIceChange, BattleMapIceReport, change_map_ice_action};

pub(crate) mod terrain_edit;
pub use terrain_edit::{BattleMapHexChange, set_map_hex_action};

pub(crate) mod map_block;
pub use map_block::add_landing_exclusion_action;
pub(crate) mod map_link;
pub(crate) mod map_load;
mod map_objects;
pub use map_load::load_map_action;
pub(crate) mod map_save;
pub use map_save::save_map_action;
mod bridge_generation;
mod map_export;
pub use map_export::BattleMapExport;
pub(crate) mod map_resize;
pub use map_resize::resize_map_action;
pub(crate) mod map_clear;
pub use map_clear::clear_map_units_action;
pub(crate) mod map_emit;
pub use map_emit::emit_map_action;

mod shutdown;
pub use shutdown::stop_unit_action;

pub(crate) mod map_decoration;
pub use map_decoration::add_map_decoration_action;

mod static_decoration;
pub use static_decoration::{
    BattleStaticDecoration, BattleStaticDecorationKind, set_static_decoration,
};

pub(crate) mod map_object_delete;
pub use map_object_delete::{BattleMapObjectKind, delete_map_objects_action};

pub(crate) mod map_list;
pub use map_list::list_map_action;

mod map_link_config;
pub use map_link_config::{BattleMapEntrance, BattleMapLink, set_map_link};

pub(crate) mod map_update_links;
pub use map_update_links::{BattleMapLinkUpdate, update_map_links_action};

pub(crate) mod map_fields;
pub use map_fields::set_map_field_action;

pub(crate) mod map_check;
pub use map_check::{BattleMapCheck, check_map_action};

pub(crate) mod map_field_report;
pub use map_field_report::{BattleMapField, BattleMapFieldReport, view_map_fields_action};

pub(crate) mod xp_ranking;
pub use xp_ranking::{BattleXpRank, BattleXpRanking, xp_ranking_action};

pub(crate) mod database_save;
pub use database_save::request_database_save;

pub(crate) mod forms_report;
pub use stock_selection::{BattlePartForm, part_catalogue, part_forms};

mod operator_settings;
pub use operator_settings::{edit_skill_threshold, edit_weapon_settings};

pub(crate) mod gunner_station;
pub use gunner_station::{
    BattleGunnerAim, BattleGunnerContext, BattleGunnerStation, gunner_context,
    gunner_station_action, register_gunner_station,
};

mod combat_operator;

mod field_bits;
mod field_report;
pub(crate) mod gunner_fields;
mod preference_fields;
pub use gunner_fields::{set_gunner_field, view_gunner_fields};

pub(crate) mod runtime_stats;
pub(crate) mod simulation_pending;
pub use runtime_stats::{BattleRuntimeStats, runtime_stats};

pub(crate) mod scenario_team;
pub use scenario_team::set_team_action;

pub(crate) mod losemit;
pub use losemit::losemit_action;

pub(crate) mod scenario_damage;
pub use scenario_damage::{BattleScenarioDamage, BattleScenarioHit, damage_section_action};

pub(crate) mod scenario_packets;
pub use scenario_packets::{BattleScenarioSalvo, BattleScenarioSalvoReport, damage_action};

pub(crate) mod weight_report;
pub use weight_report::{BattleWeightEntry, BattleWeightReport, weight_action, weight_report};

pub(crate) mod scenario_position;
pub use scenario_position::{
    BattleScenarioPosition, BattleScenarioPositionReport, set_coordinates_action,
};

pub(crate) mod battlefield_identity;
pub use battlefield_identity::assign_battlefield_id;

pub(crate) mod scenario_map;
pub use scenario_map::{BattleMapAssignment, reassign_map, remove_map_membership};

pub use scenario_map::{BattleMapIndexReport, set_map_index_action};

pub(crate) mod preferred_identity;
pub use preferred_identity::{
    BattlePreferredId, preferred_id, set_preferred_id, set_preferred_id_action,
};

mod orbital_drop;
mod orbital_drop_combat;
mod orbital_drop_movement;
mod orbital_drop_state;
pub use orbital_drop::{
    BattleDropBreach, BattleDropInterception, BattleDropLanding, BattleDropLandingInput,
    BattleDropProtection, BattleDropSurface, BattleOrbitalDrop, BattleOrbitalDropStep,
    ORBITAL_DROP_ALTITUDE,
};

pub(crate) mod orbital_drop_launch;
pub use orbital_drop_launch::{
    BattleOrbitalInsertion, initiate_action as initiate_orbital_drop_action,
};

mod cockpit_links;
mod hardware_settings;
mod targeting_mode;
pub(crate) mod unit_fields;
mod unit_identity;
pub use unit_fields::{
    BattleUnitField, BattleUnitFieldReport, set_unit_field_action, view_unit_fields_action,
};

mod jump_thrust;

pub(crate) mod special_fields;

mod display_name;
pub use display_name::display_name;

mod auxiliary_preferences;
pub use auxiliary_preferences::{set_bth_debug, set_mw_safety};
pub(crate) mod safety;

mod artillery_prediction;
pub use artillery_prediction::{BattleArtilleryPrediction, predict_artillery_target};

pub(crate) mod snipe;
pub use snipe::snipe_action;

mod damage_field;
mod damage_records;
mod damage_replacement;
pub use damage_field::unit_damage_field;
pub use damage_records::{BattleDamageRecord, parse_damage_field};
pub use damage_replacement::{BattleDamageReplacement, BattleDamageSlot, prepare_damage_field};

mod base_movement_fields;

mod engine_sink_override;

mod status_fields;

mod motion_fields;

mod template_speed;

mod registered_unit_defaults;
pub use registered_unit_defaults::{
    ensure_registered_unit_runtime, registered_unit_default_template,
};

mod raw_template;
pub use raw_template::{
    RawMovement, RawSectionCode, RawTemplate, RawUnitClass, raw_default_mech_criticals,
};
mod administrative_raw;
pub use administrative_raw::{AdministrativeRawSection, AdministrativeRawUnit};

mod propulsion;

mod status_edits;

mod live_mass;

mod construction_fields;

mod critical_conditions;

mod damage_application;

mod damage_material;

mod damage_recalculation;

mod damage_weapons;

mod component_failure;
pub use component_failure::BattleComponentFailure;

pub(crate) mod markings;
pub use markings::{set_unit_markings, unit_markings, view_unit_markings};

mod sixth_sense;
pub use sixth_sense::{advance_sixth_sense, advance_sixth_sense_action};

mod sensor_report;
pub use sensor_report::sensor_report;

pub(crate) mod turn_clock;

mod periodic_piloting;
pub use periodic_piloting::{
    BattlePeriodicPiloting, advance_periodic_piloting, advance_periodic_piloting_action,
};

pub(crate) use jumping::{launch_dfa_action, launch_jump_action};

mod menu;

mod advantages;
pub use advantages::{
    BATTLE_ADVANTAGES, BattleAdvantageDefinition, BattleAdvantageKind, advantage_definition,
};

mod spotter_events;
pub use spotter_events::{BattleSpotterEvents, advance_spotter_links};

mod shot_counters;

mod damage_counters;
mod kill_counters;

mod sprint;

mod speed_bonus;

pub(crate) use speed_bonus::SpeedPolicy;

mod roll_history;
mod roll_statistics;
pub use roll_statistics::BattleRollStatistics;

mod computer_failure;
pub use computer_failure::{
    BattleComputerFailure, BattleComputerFailureInput, select_computer_failure,
};

pub(crate) mod computer_runtime;
pub use computer_runtime::advance_battle_computer_failures_action;

mod tactical;
pub use tactical::{
    MAX_TACTICAL_UNITS, TacticalContact, TacticalIntention, TacticalSighting, TacticalSnapshot,
    TacticalSubmitResult, TacticalUnitSnapshot, observe_tactical, submit_tactical,
};
