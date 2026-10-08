//! BattleTech domain models, asset codecs, and command/notification adapters.
pub mod autopilot;
pub(crate) mod map_lifecycle;
mod special_commands;
pub(crate) mod special_dispatch;
mod special_help;
mod special_registration;
pub(crate) use special_registration::unregister_special;
pub(crate) mod unit_lifecycle;
pub use special_commands::{CommandClass, SpecialCommand, SpecialType};
pub(crate) mod electronics;
pub use electronics::{
    ElectronicSuite, Electronics, electronic_field, electronic_fields_pending,
    refresh_electronic_fields, toggle_electronics,
};
mod electronic_field;
pub use electronic_field::{
    ElectronicField, ElectronicMode, ElectronicSource, resolve_electronic_field,
};
mod ammunition_feed;
mod artemis;
mod artillery;
mod artillery_adjustment;
mod artillery_feedback;
mod artillery_firing;
pub use artillery_firing::ArtilleryLaunchReport;
mod coordinate_launch;
pub use coordinate_launch::LaunchMisload;
mod artillery_aim;
pub use artillery_aim::ArtilleryAiming;
pub use artillery_aim::{
    ArtilleryAim, ArtilleryAimInput, ArtilleryObserver, ArtilleryRange,
    unit_artillery_gunnery_target,
};
mod artillery_impact;
mod artillery_queue;
pub use artillery::{
    ArtilleryCell, ArtilleryEffect, ArtilleryFlight, ArtilleryImpactPattern, ArtilleryMode,
};
pub use artillery_impact::{ArtilleryHit, ArtilleryImpactReport, advance_artillery_flight};
pub use artillery_queue::{
    ArtilleryShot, advance_artillery_action, artillery_pending, enqueue_artillery,
};
pub use autopilot::{
    AutopilotConfig, AutopilotConfigPatch, AutopilotController, AutopilotFeedback,
    AutopilotFeedbackEvent, AutopilotFeedbackPage, AutopilotFireMode, AutopilotLastSighting,
    AutopilotOrder, AutopilotOrderProgress, AutopilotOrderRecord, AutopilotOrderState,
    AutopilotRangeBand, AutopilotReason, AutopilotState, AutopilotSubmissionMode, LastSighting,
    MAX_FEEDBACK,
};
pub use evacuation::advance_artillery_flight_action;

pub use ammunition_feed::AmmunitionDraw;
pub(crate) mod pods;
pub(crate) mod vehicle_pods;
pub use pods::{PodRemoval, PodRow, inspect_pods, pod_status, remove_pod, remove_pod_action};
pub use vehicle_pods::{begin_pod_removal, begin_pod_removal_action};
pub(crate) mod inarc;
pub use inarc::set_inarc_ammunition;
pub(crate) mod narc;
pub(crate) use narc::BeaconLaunch;
mod vehicle_narc;
pub use narc::{BeaconKind, NarcReport, UnitSection, toggle_explosive, toggle_narc};
pub(crate) mod ams;
mod ams_unit;
pub use ams::{AmsReport, set_ams};
pub(crate) mod unjam;
pub use unjam::{Unjam, advance_unjamming, advance_unjamming_action, begin_unjam};
mod weapon_jam;
pub use artemis::ArtemisController;
mod gyro;
pub use gyro::Gyro;
mod engine;
mod mass;
pub use mass::Mass;
mod stacking;
pub use stacking::{StackingEntry, StackingInput, StackingRules, resolve_stacking};
mod preferences;
mod water_movement;
pub use preferences::set_auto_fall;
mod aim;
mod arcs;
pub use arcs::MountArcs;
mod assets;
pub use aim::{
    AimModifiers, AimRules, IndirectAim, PerceptionAim, aim_modifiers, pilot_aim_modifiers,
    unit_target_movement_modifier,
};
pub use arcs::{ContactArc, Facing, Torso, flip_arms, rotate_torso, weapon_bears_on};
mod surface_break;
pub use surface_break::{Surface, SurfaceBreak, break_bridge, break_ice, collapse_structure};
mod balance;
pub use balance::{BalanceCause, BalanceReport};
mod character;
pub(crate) mod character_clear;
pub(crate) mod character_list;
mod character_names;
pub(crate) mod character_show;
pub use character_clear::clear_character;
pub(crate) mod commands;
pub(crate) mod firing;
pub use character::{
    Character, CharacterInjury, ConsciousnessCheck, injure_character, set_character,
};
mod crew;
mod critical;
pub use critical::{AmmunitionHazard, CriticalLoss, destroy_unit_critical};
mod damage;
mod dice;
pub use damage::{DamagePhase, DamageResult, apply_damage_phase};
pub(crate) use dice::DiceState;
pub use dice::{Dice, roll_unit_dice};
pub(crate) mod fire_mode;
pub use fire_mode::{toggle_flamer_heat, toggle_hotload};
mod geometry;
mod heat;
mod overheat;
pub use heat::{Heat, HeatRates, advance_heat};
pub use overheat::{HeatCheck, OverheatClock, OverheatReport, OverheatRules, advance_overheat};
mod free_fall;
mod hit;
mod impact;
mod jump;
mod jump_fields;
mod jump_flight;
pub use free_fall::{FreeFall, FreeFallStep};
pub(crate) mod jumping;
pub use hit::{Hit, HitArc, HitRules, HitTable};
pub use impact::{CrewCasualty, ImpactEffect, ImpactReport, explode_ammunition, resolve_impact};
pub use jump::{JumpCapacity, JumpPath, JumpSample};
pub use jump_flight::{JumpFlight, JumpOutcome, JumpStep};
pub use jumping::{advance_jumps, launch_jump};
pub(crate) mod landing;
pub use landing::land_jump;
mod ground_proposal;
mod loadout_context;
mod map_flags;
pub use stompymux_map::{MapFlag, format_map_flags, parse_map_flags};
mod mobility;
mod motion;
pub use ground_proposal::{
    GroundMotionProposal, propose_mech_ground_motion, propose_vehicle_ground_motion,
};
mod movement_report;
mod template_check;
pub use mobility::Mobility;
pub use template_check::{AmmunitionAdjustment, TemplateCheck, check_template};
mod pilot_health;
mod pilot_injury;
mod placement;
pub use pilot_injury::{
    TacticalImpact, TacticalPilotInjury, injure_tactical_pilot, resolve_tactical_impact,
};
mod power;
mod recovery;
mod recovery_output;
pub use recovery::{
    CharacterNotice, Recovery, RecoveryMode, advance_recovery, check_character_consciousness,
    prepare_recovery,
};
mod readiness;
mod weapon_admission;
pub use readiness::{WeaponReadiness, WeaponUse, advance_recycle, spend_weapon};
mod salvo;
pub use salvo::WeaponSalvo;
mod state;
mod stun;
pub use salvo::{MechSalvoReport, SalvoGroup, SalvoReport, resolve_salvo, resolve_tactical_salvo};
pub use stun::{advance_stun, stun_unit};
mod chassis_actions;
mod unit;
pub use chassis_actions::{
    ByChassis, UnitFallReport, UnitShotReport, fire_unit_shot, resolve_unit_fall,
};
mod unit_access;
pub(crate) use unit_access::{with_unit, with_unit_mut};
mod unit_validation;
pub use unit_access::{UnitDefinition, UnitEdit, UnitMut, UnitRef};

pub use assets::{read_map, read_template, read_unit_template, read_vehicle_template};
mod unit_template;
pub use state::{
    BtechState, StoredBattleUnit, StoredMap, create_map, create_unit, register_empty_battle_unit,
    reload_map, set_map_visibility,
};
pub use stompymux_map::{
    AttackKind, Condition, ConstructionClass, DecorationKind, Density, Flow, Foliage, Ground,
    GroundMovement, Hex, MAX_CONSTRUCTION_FACTOR, MAX_DEPTH, MAX_HEIGHT, MAX_VISIBILITY, MapAsset,
    MapPointOfInterest, MapRegion, Route, Structure, StructureKind, Terrain, Water, Wind,
    WindStrength, extreme_temperature_steps, gravity_aim_modifier,
};
pub use unit_template::UnitTemplateExt;
mod vehicle;
mod vehicle_driving;
mod vehicle_motion;
pub use vehicle_motion::VehicleMotionRules;
pub use vehicle_motion::VehicleTemplateMotion;
mod vehicle_placement;
mod vehicle_power;
pub use vehicle::{Vehicle, create_vehicle, damage_vehicle_motive, damage_vehicle_phase};
mod engine_sink_capacity;
mod mode_feedback;
mod vehicle_mass;
pub use mode_feedback::{AmmunitionFeedback, FireModeFeedback};

pub use placement::{place_unit, remove_unit};
pub use unit::{Mech, Position, SectionState};

pub(crate) use crew::player_moved;
pub use crew::{assign_pilot, release_pilot};

pub use power::{Notice, Power, advance_units, start_unit, stop_unit};

/// Render a domain notice through ordinary container notification, staging it until commit.
pub(crate) fn notify_unit(scripts: &crate::Scripts, notice: Notice) -> anyhow::Result<()> {
    notify_unit_text(scripts, notice.unit, &notice.text)
}

/// Stage a formatted cockpit message through the same transaction as domain notices.
pub(super) fn notify_unit_text(
    scripts: &crate::Scripts,
    unit: crate::ObjectId,
    text: &str,
) -> anyhow::Result<()> {
    notify_message(scripts, MessageTarget::Unit(unit), text)
}

/// Stage a typed recipient without accidentally expanding private pilot details to cockpit contents.
pub(crate) fn notify_message(
    scripts: &crate::Scripts,
    recipient: MessageTarget,
    text: &str,
) -> anyhow::Result<()> {
    let world = scripts.world.borrow();
    let config = crate::lua::configuration(&scripts.lua);
    let (targets, policy) = match recipient {
        MessageTarget::Unit(id) => {
            let mut targets = vec![id];
            targets.extend(cockpit_links::audiences(&world, id));
            (targets, crate::notification::Policy::ROOM)
        }
        MessageTarget::Player(id) => (vec![id], crate::notification::Policy::DIRECT),
    };
    for target in targets {
        crate::notification::send(
            &world,
            &scripts.outbox,
            &config,
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
}

pub use geometry::{Range, unit_elevation, unit_range};
pub use stompymux_map::{HexCoordinate, Point};

pub use stompymux_unit_construction::{
    AMMUNITION_PART_OFFSET, AmmunitionBin, AmmunitionMode, CriticalDefinition, CriticalLocation,
    DamageClass, Engine, FireMode, InspectionArmor, InspectionCritical, InspectionPart,
    InspectionWeapon, MechChassis, MechLoadout, MechSection, MechTemplate, PART_ID_LIMIT,
    ParsedTemplate, Part, PartForm, PartKind, PartNames, PartPrices, RangeBracket, RawMovement,
    RawSectionCode, RawTemplate, RawUnitClass, ResolvedLoadout, SectionDefinition, System,
    SystemCritical, Technology, TemplateRegistryCache, UnitTemplate, VehicleCriticalLocation,
    VehicleEngine, VehicleLoadout, VehicleMass, VehicleMaterial, VehicleMovement,
    VehiclePowerplant, VehicleSection, VehicleTemplate, WEAPON_PART_IDS, WaterRanges, Weapon,
    WeaponMount, WeaponProfile, WeaponRange, administrative_technology,
    administrative_template_movement, administrative_template_tonnage, armor_mass,
    cargo_space_mass, edit_special, engine_mass, finalize_raw_load_specials,
    flag_spells_technology, half_ton, inspect_raw_template_armor, inspect_raw_template_criticals,
    inspect_raw_template_engine, inspect_raw_template_weapons, inspection_ammunition_modes,
    inspection_canonical_mech_internal, inspection_compatible_template,
    inspection_compatible_vehicle_template, inspection_configured_technology,
    inspection_configured_technology_attributes, inspection_engine_rating, inspection_fire_modes,
    inspection_normalized_jump_speed, inspection_raw_part, inspection_template_part,
    inspection_vehicle_engine_rating, inspection_vehicle_engine_values, mixed_technology_flag,
    one_shot_mass, parse_engine_sink_override, parse_template_speed, part_abbreviation,
    part_catalogue, part_names, part_price, part_short_name, power_amplifier_mass, rated_output,
    raw_default_mech_criticals, raw_template_base_cost, read_engine_sink_override,
    read_resolved_raw_template, read_resolved_template, read_template_document,
    read_template_speed, reflective_armor_slots, resolve_template_path,
    resolve_template_path_bytes_cached, resolve_template_path_cached, strip_name_prefix,
    structure_mass, system_slot_mass, template_base_cost, unit_metadata, validate_unit_metadata,
    vehicle_template_base_cost, write_engine_sink_override, write_template, write_template_speed,
};

pub(crate) use motion::set_speed_autopilot;
pub use motion::{Motion, MovementRules, advance_motion, set_heading, set_speed};

/// Stage a player-owned condition notice through the normal notification boundary.
pub(crate) fn notify_character(
    scripts: &crate::Scripts,
    notice: CharacterNotice,
) -> anyhow::Result<()> {
    recovery_output::publish(scripts, notice)
}

mod los;
mod los_trace;
pub use los::{TerrainLos, ground_terrain_los, unit_terrain_los};

mod light_aim;
mod map_light;
pub use stompymux_map::Light;

mod perception;
pub use perception::{
    AUTOMATIC_DETECTION_RANGE, AcquisitionRules, ActiveProbe, DEFAULT_SENSOR_RANGE, Detection,
    DetectionChannel, HIDDEN_DETECTION_RANGE, MapPerceptionFlag, Perception, PerceptionProfile,
    PerceptionReport, PerceptionStatus, ProbeProfile, RADAR_RANGE, RadarProfile, RadarTarget,
    SensorArc, SensorRange, configure_perception, format_perception_flags, hex_perception,
    parse_perception_flags, perceive, perception_factor, perception_profile, perception_report,
    set_map_perception,
};

mod contacts;
pub use contacts::{
    Contact, ContactRules, ContactTransition, ContactUpdate, ContactView, displayed_contact,
    displayed_contacts, update_contact, visible_contact, visible_contacts,
};

mod skills;
pub use skills::{
    CharacterValue, SkillCategory, gunnery_target, perception_target, set_character_value,
    unit_gunnery_target, unit_piloting_target,
};

mod scanner;
pub(crate) use scanner::notify_contact;
pub use scanner::{
    ContactEvent, UnitSignature, contact_observers, refresh_contacts, set_unit_signature,
};

mod targeting;
pub use targeting::{TargetLock, advance_target_locks, select_target};

pub(crate) mod fire_target;
pub use fire_target::FireTarget;
mod equipment_context;
mod shot;
mod shot_transaction;
mod validation_contacts;
mod validation_context;
pub use shot::{GlancingMode, MechShotReport, RecoilReport, ShotReport, ShotRules, resolve_shot};

mod magma;
mod piloting;
mod planetary_conditions;
mod terrain_entry;
pub use magma::{MagmaSnapshot, advance_magma_action, magma_snapshot};
mod vehicle_arcs;
pub use vehicle_arcs::VehicleMountArcs;
mod vehicle_control_damage;
pub use vehicle_control_damage::{VehicleControlHit, damage_vehicle_controls};
mod vehicle_critical_table;
pub use vehicle_critical_table::{
    VehicleCriticalEffect, VehicleCriticalReport, VehicleCriticalRules, VehicleCriticalTable,
    roll_vehicle_critical,
};
mod vehicle_critical;
pub use vehicle_critical::{destroy_vehicle_critical, select_vehicle_weapon_critical};
mod vehicle_readiness;
pub use vehicle_readiness::{VehicleWeaponUse, reserve_vehicle_weapon};
mod vehicle_hit;
mod vehicle_piloting;
mod vehicle_turret;
pub use piloting::{PilotNotice, PilotingCheck, roll_piloting};
pub use vehicle_hit::{VehicleHit, VehicleHitRules, VehicleMotiveHit};
pub use vehicle_turret::{lock_vehicle_turret, set_turret, turret_readout};

mod fall;
pub use fall::{FallReport, FallRules, MechFallFeedback, MechFallReport, Posture, resolve_fall};

pub(crate) mod stand;
pub use stand::{StandAttempt, StandMode, StandTimer, advance_standing, begin_stand, stand_target};

mod stagger;
pub use stagger::{Stagger, StaggerHit, StaggerMode, StaggerReport, StaggerRules, advance_stagger};

mod flooding;
pub use flooding::flood_unit;
mod section_exposure;
pub use section_exposure::{SectionExposure, SectionExposureReport};

pub(crate) mod ammunition_mode;
pub use ammunition_mode::{toggle_artemis, toggle_cluster, toggle_lbx};

pub(crate) mod ultra;
pub use ultra::toggle_ultra;

pub(crate) mod rapid;
pub use rapid::toggle_rapid;

pub(crate) mod rotary;
pub(crate) use rotary::RotaryDamage;
pub use rotary::set_rotary;

pub(crate) mod gatling;
pub use gatling::toggle_gatling;

mod weapon_controls;

pub(crate) mod precision;
pub(crate) use precision::PrecisionAim;
pub use precision::toggle_precision;

pub(crate) mod flechette;
pub(crate) use flechette::FlechetteDamage;
pub use flechette::toggle_flechette;

pub(crate) mod armor_piercing;
pub(crate) use armor_piercing::ArmorPiercing;
pub use armor_piercing::toggle_armor_piercing;

pub(crate) mod caseless;
pub use caseless::toggle_caseless;

pub(crate) mod incendiary;
pub use incendiary::toggle_incendiary;

mod broadcast;
pub use broadcast::{MessageTarget, observer_messages};

mod searchlight;
pub use searchlight::{
    Searchlight, SearchlightMode, advance_searchlights, set_searchlight_mode, toggle_searchlight,
    unit_illuminated,
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
    Arm, ArmAttackReport, ArmRejection, ArmSelection, LancePenetration, Leg, PhysicalAttack,
    PhysicalProfile, PhysicalReport, PhysicalRules, kick_profile, punch_profile, resolve_kick,
    resolve_punch, resolve_trip, trip_profile,
};

mod myomer;

pub use physical::{ArmAttack, arm_attack_profile, resolve_arm_attack};

mod club;
pub use club::grab_club;
pub use physical::{club_profile, resolve_club};

mod charge;
pub use charge::{
    ChargeBalance, ChargeProfile, ChargeReport, ChargeRules, charge_profile, resolve_charge,
};

pub use charge::{MutualChargeAttempt, MutualChargeReport, resolve_mutual_charge};

mod charge_tracking;
pub use charge_tracking::{ChargePolicy, ChargeSelection, ChargeState, select_charge};

mod dfa;
pub use dfa::{DfaBalance, DfaProfile, DfaReport, dfa_profile, resolve_dfa};

pub use jumping::launch_dfa;

mod experience;
mod physical_experience;
pub use experience::{ExperienceAward, ExperienceRules, award_character_experience};

pub(crate) mod skill_catalog;
pub use skill_catalog::{
    BATTLE_SKILLS, SkillDefinition, SkillProgress, award_skill_experience,
    retain_character_experience, set_skill_threshold, skill_definition, skill_progress,
    skill_threshold,
};

mod evacuation;
pub(crate) use evacuation::evacuate;

pub use evacuation::resolve_impact_action;

mod character_pilot;
pub use character_pilot::{CharacterPilotInjury, CharacterPilotStatus, injure_character_pilot};

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
pub use physical::{ArmAttackChoice, ArmWeapon};

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
pub use gunnery_experience::{GunneryExperienceChance, GunneryExperienceInput};

pub use gunnery_experience::{
    GunneryExperienceMode, UnitExperience, gunnery_experience_eligible, set_unit_experience,
};

pub use gunnery_experience::{
    GunneryAwardRequest, GunneryExperienceAward, award_classic_gunnery_experience,
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

pub use gunnery_experience::{ShotExperienceAward, award_gunnery_experience};

mod diagnostics;
use crate::logging::TraceTopic;
pub use diagnostics::DiagnosticMessage;

mod movement_experience;
pub use movement_experience::MovementExperience;

pub use evacuation::land_action;

mod stealth;
pub use stealth::StealthRange;
pub use stealth::{advance_stealth, toggle_stealth};

mod signature;
pub use signature::{
    SignatureState, SignatureTransition, advance_null_signature, toggle_null_signature,
};

mod recent_fire;
pub(crate) mod saved_parts;
pub(crate) mod timers;
mod unit_timers;
mod vehicle_timers;
pub use recent_fire::clear_recent_fire;

mod tag;
pub use tag::{TagState, advance_tags, select_tag, tagged_by};

pub(crate) mod semiguided;
pub(crate) use semiguided::SemiGuidedAim;
pub use semiguided::toggle_semiguided;
pub(crate) mod stinger;
pub use stinger::toggle_stinger;
pub(crate) mod thunder;
pub use thunder::{THUNDER_MAXIMUM_STRENGTH, ThunderField, ThunderReport, toggle_thunder};
pub(crate) mod torpedo;

mod spotter;
pub use spotter::{SpotterTarget, select_spotter, spotter_target};

mod hex_visibility;
pub use hex_visibility::{hex_detection, hex_visible};

pub use targeting::{HexLock, HexTargetMode, TargetSelection, select_hex_target};

pub use targeting::hex_occupant;

mod map_slots;
pub use map_slots::map_unit_order;

mod hex_aim;
pub use hex_aim::{HexAimModifiers, hex_aim_modifiers, pilot_hex_aim_modifiers};

mod launch_roll;
mod weapon_launch;

mod woodland;
pub use woodland::TerrainIgnition;
pub use woodland::{WoodlandClearing, WoodlandEffect, WoodlandIntent, resolve_woodland_effect};

mod woodland_map;
pub use woodland_map::{WoodlandChange, apply_woodland_clearing};

mod decorations;
pub use decorations::{Decoration, advance_map_smoke, map_smoke_pending, set_map_decoration};

mod wind;
pub use wind::set_map_wind;

mod fire_spread;
pub use fire_spread::{advance_map_fire, map_fire_pending};

mod woodland_action;
mod woods_absorption;
pub use woodland_action::{WoodlandAttack, WoodlandImpact, resolve_woodland_attack};
pub use woods_absorption::WoodsAbsorption;

mod weapon_groups;

mod hex_shot;
pub use hex_shot::{HexShotReport, resolve_hex_shot};

mod fire_report;
pub use fire_report::FireReport;
mod direct_effects;
mod fire_feedback;
mod hex_firing;
mod launch_feedback;
mod vehicle_firing;

mod surface_weapon;
pub use surface_weapon::SurfaceWeaponImpact;

mod building;
pub use building::{BuildingState, set_building_state};

mod building_entrance;
pub use building_entrance::{BuildingEntrance, set_building_entrance};

mod building_damage;
pub use building_damage::{BuildingImpact, advance_building_repairs, building_repair_pending};

pub(crate) mod map_view;
pub use map_view::view_map_action;
pub(crate) mod map_mine;
pub use map_mine::{MinePlacement, add_mine_action};
mod minefield;
pub use minefield::{MineKind, Minefield, insert_minefield, set_minefield};

mod mine_activation;
pub use mine_activation::{MineActivation, MineResponse, MineTriggerReason, mine_activations};

mod blast_damage;
pub use blast_damage::BlastImpact;
mod mine_blast;
pub use evacuation::resolve_mine_blast_action;
pub use mine_blast::{MineBlastHit, MineBlastReport, resolve_mine_blast};

mod inferno;
pub use inferno::{advance_inferno_burns, apply_inferno_burn, extinguish_inferno_in_water};

mod inferno_hit;
pub use inferno_hit::{InfernoHit, resolve_inferno_hit};

pub(crate) mod inferno_ammunition;
pub use inferno_ammunition::toggle_inferno;

mod mine_event;
pub use mine_event::{MineEventReport, activate_mines};

mod command_mines;
pub use command_mines::{CommandMineReport, detonate_command_mines};
pub use evacuation::detonate_command_mines_action;

pub(crate) mod radio;
pub use radio::{
    RadioCapabilities, RadioChannel, RadioMode, set_radio_frequency, set_radio_mode,
    set_radio_title,
};

mod radio_delivery;
pub use radio_delivery::{DigitalRadioReport, RadioReception, resolve_digital_radio};

mod radio_analog;
pub use radio_analog::{AnalogRadioReport, resolve_analog_radio};

pub(crate) mod radio_transmission;
pub use radio_transmission::{RadioDelivery, RadioTransmission, send_radio_action};

mod radio_audit;
pub use radio_audit::set_radio_frequency_action;

mod radio_experience;
pub use radio_experience::award_radio_experience;

mod observer;
pub use observer::{set_observer, unit_observer};

pub(crate) mod radio_targeted;
pub use radio_targeted::{TargetedRadioReport, resolve_targeted_radio, send_targeted_radio_action};

pub(crate) mod scan;
mod scan_summary;
mod scan_weapons;
pub use scan::{SensorRanges, scan_hex_unit_action, scan_unit, scan_unit_action};

mod scan_building;
pub use scan_building::{BuildingScan, scan_building, scan_building_action};

mod perception_check;
mod scan_mines;
pub use scan_mines::{HexScan, MineScan, scan_hex_action, scan_mines};

mod scan_selected;
pub use scan_selected::{SelectedScan, scan_selected_action};

pub(crate) mod report;
pub use scan::report_unit;

mod view_center;
pub use view_center::{ViewCenter, ViewKind, ViewPosition, parse_view_center, resolve_view_center};

mod viewport;
pub use viewport::{ViewDimensions, Viewport, resolve_viewport};

pub(crate) mod long_range_map;
pub use long_range_map::{LongRangeMap, LongRangeMode, long_range_map};

mod map_style;

mod hex_illumination;
pub use hex_illumination::hex_illuminated;

pub(crate) mod tactical_map;
pub use tactical_map::{TacticalMap, tactical_map};

mod landing_zone;
pub use landing_zone::{LandingExclusion, LandingSuitability, set_landing_exclusion};

pub(crate) mod find_center;
pub use find_center::{HexCenterReport, find_center};

pub(crate) mod navigation;
pub use navigation::{NavigationReport, navigate};

pub(crate) mod view_preferences;
pub use view_preferences::{PlayerPreferences, set_view_dimensions, view_dimensions};
mod player_configuration;
pub use player_configuration::{
    PersonalEquipment, PersonalLoadout, PlayerConfiguration, player_configuration,
    set_player_configuration,
};
mod inspection;
pub use inspection::{
    InspectionTechnology, compose_unit_raw_inspection, compose_vehicle_raw_inspection,
    inspect_composed_unit_armor, inspect_composed_vehicle_armor, inspect_raw_template_battle_value,
    inspect_raw_template_inventory, inspect_raw_template_technologies, inspect_section_condition,
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
    inspection_battle_value, inspection_effective_maximum_speed, inspection_section,
    inspection_section_code, inspection_template_battle_value, inspection_vehicle_section,
    inspection_vehicle_section_code, inspection_vehicle_section_for,
    inspection_vehicle_template_battle_value,
};
mod unit_operations_contract;
pub(crate) use unit_operations_contract::{
    UnitDamageRequest, apply_unit_damage_action, configure_unit_ammunition,
    configure_vehicle_ammunition, install_unit_special, install_unit_weapon_named,
    install_vehicle_special, install_vehicle_weapon_named, load_unit_template,
    reset_unit_criticals, restock_unit_ammunition, restock_vehicle_ammunition,
    set_unit_weapon_modes, unit_piloting_check_action,
};
mod admin_contract;
pub use admin_contract::{
    AdministrativeRepairKind, ReattachHull, administrative_assigned_pilot,
    administrative_is_fixable, administrative_section_info, administrative_section_valid,
    administrative_unit_class, administrative_unit_movement, administrative_unit_tonnage,
    apply_administrative_repair, clear_administrative_technologies, set_administrative_armor,
    set_administrative_assigned_pilot, set_administrative_cargo, set_administrative_heat_sinks,
    set_administrative_movement_type, set_administrative_radio_quality, set_administrative_scalar,
    set_administrative_technology, set_administrative_unit_type,
};
mod unit_configuration;
pub use unit_configuration::{
    UnitConfiguration, set_unit_configuration, set_unit_identity_configuration, unit_configuration,
};
mod event_telemetry;
pub use event_telemetry::EventTelemetry;
mod character_value_contract;
pub use character_value_contract::{
    CharacterValueDefinition, character_raw_value, character_saved_value,
    character_value_definition, character_value_definition_code, character_value_definitions,
    set_character_raw_value, set_character_skill_experience, set_character_skill_target,
};
mod lua_map_contract;
pub use lua_map_contract::{
    MapEmitAudience, MapLos, MapMember, MapSpatialPoint, emit_battle_map_trusted_action,
    load_battle_map_trusted_action, map_hex_los, map_hex_point, map_members, map_spatial_range,
    map_unit_by_label, map_unit_los, map_unit_map, map_unit_point, place_battle_map_unit,
    update_battle_map_links_trusted_action,
};

mod contact_preferences;
pub(crate) mod contact_report;
pub use contact_preferences::{
    BuildingContactMode, ContactPreferences, contact_preferences, filtered_contacts,
    set_contact_preferences,
};

pub use contact_preferences::{ContactOptions, parse_contact_options};

mod building_contacts;
pub use building_contacts::{BuildingContact, building_contacts};

pub(crate) mod brief;
pub use brief::{BriefReport, BriefSettings, brief};

pub use contact_preferences::parse_contact_options_for_display;

mod contact_status;

pub use preferences::set_autocon_shutdown;

pub(crate) mod lateral;
pub use lateral::{LateralMode, LateralState, lateral, set_lateral};

pub(crate) mod bootlegger;
pub use bootlegger::{BootleggerReport, bootlegger, bootlegger_modifier};

pub(crate) mod eta;
pub use eta::{EtaReport, eta, eta_action};

pub(crate) mod bearing;
pub use bearing::{BearingReport, bearing};

mod navigation_measurement;
pub(crate) mod range_display;
pub use range_display::{RangeReport, range_display};

pub(crate) mod vector_display;
pub use vector_display::{VectorReport, vector_display};

pub(crate) mod motion_controls;
pub use motion_controls::motion_readout;

pub(crate) mod dumping;
pub use dumping::{Dump, DumpIgnition, DumpSelection, advance_dumping, begin_dump, dump};

mod boosters;

pub(crate) mod booster_control;
pub use booster_control::{
    BoosterState, advance_boosters_action, masc, supercharger, toggle_masc, toggle_supercharger,
};

mod c3_hardware;
mod network_unit;
pub use c3_hardware::C3Hardware;

pub(crate) mod command_network;

mod network_range;
pub use network_range::NetworkRange;

pub(crate) mod network_message;

pub(crate) mod network_status;

pub(crate) mod network_topology;
pub use network_topology::{NetworkAutomation, reconcile as reconcile_command_networks};

pub(crate) mod network_contacts;

mod radio_scanning;
pub use radio_scanning::FrequencyScan;

mod chassis;

mod motion_speed;

mod leg_support;

mod weapon_failure;
pub use weapon_failure::{EquipmentFailure, set_weapon_failure};
mod vehicle_weapon_failure;
pub use vehicle_weapon_failure::{VehicleWeaponJam, jam_vehicle_weapon};

pub use vehicle_turret::{begin_vehicle_turret_repair, jam_vehicle_turret};

mod vehicle_critical_resolution;
pub use vehicle_critical_resolution::{VehicleCriticalResolution, resolve_vehicle_critical};

pub use vehicle_weapon_failure::{VehicleMainWeaponJam, jam_vehicle_main_weapon};

mod vehicle_ammunition_cascade;
pub use vehicle_ammunition_cascade::{
    VehicleAmmunitionCascade, discharge_vehicle_ammunition_cascade,
};

mod vehicle_internal_damage;
pub use vehicle_internal_damage::{VehicleInternalDamage, resolve_vehicle_internal_damage};

mod vehicle_explosion;
pub use vehicle_explosion::VehicleExplosion;

mod vehicle_armor_damage;
pub use vehicle_armor_damage::{VehicleArmorDamage, VehicleArmorHit, resolve_vehicle_armor_damage};

mod vehicle_impact;
pub use vehicle_impact::{VehicleImpact, VehicleImpactRules, resolve_vehicle_impact};

mod vehicle_aim;

mod vehicle_shot;
pub use vehicle_shot::check_vehicle_shot;

mod unjam_unit;
mod vehicle_launch;
pub use vehicle_launch::{VehicleLaunch, VehicleLaunchRequest, launch_vehicle_weapon};

mod vehicle_salvo;
pub use vehicle_salvo::{
    VehicleSalvoGroup, VehicleSalvoReport, VehicleSalvoRequest, resolve_vehicle_salvo,
};

mod vehicle_fire;
pub use vehicle_fire::{VehicleShotReport, VehicleShotRules, fire_vehicle_shot};

mod target_salvo;
pub use target_salvo::TargetSalvo;

mod crew_recovery;

pub(crate) mod vehicle_burning;
pub use vehicle_burning::{
    VehicleFireTick, VehicleHeatExposure, VehicleInfernoHit, advance_vehicle_fires,
    begin_vehicle_extinguishing, begin_vehicle_extinguishing_action, resolve_vehicle_heat_exposure,
    resolve_vehicle_inferno_hit,
};

mod vehicle_injuries;
mod vehicle_motive_effects;
pub use evacuation::{
    advance_vehicle_fires_action, resolve_vehicle_fire_exposure_action,
    resolve_vehicle_heat_exposure_action, resolve_vehicle_inferno_hit_action,
};
pub use vehicle_burning::{VehicleFireEffects, VehicleFireExposure, resolve_vehicle_fire_exposure};

mod fall_profile;
mod vehicle_fall;
pub use evacuation::vehicle_fall_action;
pub use vehicle_fall::{VehicleFallFeedback, VehicleFallReport, resolve_vehicle_fall};

mod reverse_slope;

mod cliff;
mod vehicle_cliff;

mod vehicle_water;

mod terrain_control;

mod vehicle_obstacle;

mod transport_loss;

mod vtol_hit;
pub use vtol_hit::VtolHitLocation;
pub use vtol_hit::{RotorHit, VtolHit};

mod rotor_damage;
pub use rotor_damage::RotorDamage;

mod vtol_speed;

mod vtol_fuel;
pub use vtol_fuel::{VtolFuel, VtolFuelStatus, VtolFuelUse, set_vtol_fuel, vtol_fuel_status};

pub(crate) mod vtol_controls;
mod vtol_flight;
pub use vtol_controls::{begin_vtol_takeoff, set_vtol_vertical_speed, vtol_vertical_readout};
pub use vtol_flight::{VtolFlight, VtolFlightPhase, VtolTakeoff};

mod vtol_landing;
pub use vtol_landing::VtolLanding;

mod vtol_motion;
pub use vtol_motion::{VtolMotionStep, VtolSurfaceContact};

mod vtol_path;
pub use vtol_path::VtolPath;

mod vtol_environment;
mod vtol_updates;
pub use vtol_environment::{VtolEnvironment, advance_vtol_environment};
mod vtol_crash;
pub use vtol_crash::{VehicleDescentEvent, advance_vtol_fall, resolve_vtol_crash};

mod vtol_emergency;

mod building_routes;
pub use building_routes::{
    BuildingEntryPoint, BuildingExit, building_entry_destination, building_exit_destination,
    set_building_entry_point, set_building_exit, set_building_return_link,
};

mod map_transfer;
pub use map_transfer::transfer_unit;

mod building_entry;
pub use building_entry::{
    BuildingEntry, advance_building_entry, begin_building_entry, building_entry,
    building_entry_destination_for_unit, building_entry_lock_allows, clear_building_entry,
};

pub(crate) mod building_actions;
pub use building_actions::{
    BuildingArrival, advance_building_entries_action, begin_building_entry_action,
    building_entries_pending, exit_building_action, publish_building_arrivals,
};

mod building_exit;
mod building_step;
pub use building_exit::{building_exit_for_unit, exit_building};

mod towing;
pub use towing::set_tow;

mod load;
pub use load::{UnitLoad, unit_load};

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
pub use pickup_transaction::{PickupReport, pickup_unit};

pub(crate) mod tow_actions;
pub use tow_actions::tow_action;

pub(crate) mod dig;
pub use dig::{DigState, dig_action, dig_unit};

pub(crate) mod hull_down;
pub use hull_down::{HullDownState, hull_down_action, set_hull_down};

pub(crate) mod fortification;
pub use fortification::{set_fortified, unit_fortified};

pub use geometry::unit_altitude;

pub use radio::unit_radio_capabilities;

pub(crate) mod tic;
pub use tic::{TicEdit, Tics, edit_battle_tic, tic};

pub use tic::{TicShot, fire_battle_tics};

pub(crate) mod heat_cutoff;
pub use heat_cutoff::{HeatCutoff, toggle_battle_heat_cutoff};

pub(crate) mod ammunition_preference;
pub use ammunition_preference::set_battle_ammunition_section;

pub(crate) mod automatic_turret;
pub use automatic_turret::{
    advance_battle_automatic_turrets, automatic_turrets_pending, toggle_battle_automatic_turret,
};

mod reactor_explosion;
pub use reactor_explosion::{ReactorBlastHit, ReactorExplosion, reactor_explosion_action};

pub(crate) mod self_destruct;
pub use self_destruct::{
    SelfDestruct, SelfDestructOutcome, advance_battle_self_destructs_action, self_destruct_action,
    self_destructs_pending,
};

pub use self_destruct::set_battle_self_destruct_safe;

pub(crate) mod reactor_instability;
pub use reactor_instability::{
    advance_battle_reactor_windows, configure_battle_reactor_policy, reactor_windows_pending,
};

pub(crate) mod wreck_cleanup;
pub use wreck_cleanup::{advance_battle_wrecks_action, wrecks_pending};

pub(crate) mod prone;
pub use prone::{ProneReport, prone_action};

pub(crate) mod hiding;
pub use hiding::{advance_battle_hiding, begin_battle_hiding, hiding_pending};

mod weapons_hold;
pub use weapons_hold::{set_battle_weapons_hold, weapons_hold};

mod visibility;
pub use visibility::{Visibility, set_battle_visibility, visibility};

mod combat_safe;
pub use combat_safe::{combat_safe, set_battle_combat_safe};

pub(crate) mod swarm;
pub use swarm::{SwarmHop, SwarmReport, toggle_swarm};

pub(crate) mod special_rounds;
pub use special_rounds::toggle_missile_rounds;

mod weapon_damage;
pub use weapon_damage::{WeaponDamage, WeaponDamageEffects, WeaponDamageKind};

pub(crate) mod sight;
pub use sight::{SightAim, SightReport};

mod weapon_geometry;

pub(crate) mod weapon_reports;
pub use weapon_reports::{
    EquipmentCondition, WeaponDiagnostic, WeaponSpecification, weapon_diagnostic_text,
    weapon_diagnostics, weapon_specification_text, weapon_specifications,
};

pub(crate) mod critical_report;
mod equipment_display;
pub use critical_report::{CriticalInspection, CriticalReport, critical_report, critical_status};

pub(crate) mod aimed_target;
pub use aimed_target::{AimSelection, aimed_section, set_aimed_section};

mod aimed_hit;

mod hit_direction;

mod target_hit;

pub(crate) mod weapon_settings;
pub use weapon_settings::{
    WeaponSettings, WeaponValues, set_weapon_battle_value, set_weapon_recycle,
};

mod inventory;
pub use inventory::{
    InventoryEntry, inventory, part_cost, set_inventory_quantity, set_inventory_quantity_action,
    set_part_cost, set_part_store_quantity,
};

mod parts;
pub use parts::{inventory_mass, set_inventory_named};

mod cargo_bay;
pub use cargo_bay::{CargoTransferPoint, check_cargo_transfer_point, set_cargo_transfer_point};

pub(crate) mod cargo;
pub use cargo::{CargoOperation, CargoRow, cargo_manifest, transfer_cargo, transfer_cargo_action};

mod stock_selection;

pub(crate) mod stock_commands;
pub use stock_commands::{InventoryChange, add_stores_action, change_inventory_action};

pub(crate) mod weapon_power;
pub use weapon_power::disable_gauss_weapon;

pub(crate) mod mml;
mod technology;
pub use mml::toggle_mml_ammunition;

pub(crate) mod atm;
pub use atm::toggle_atm_ammunition;

pub(crate) mod inventory_cleanup;
pub use inventory_cleanup::{InventoryCleanup, clean_inventory, clean_inventory_action};

mod clouds;
pub(crate) mod map_environment;
pub use clouds::set_map_cloud_base;
mod vacuum;
pub use map_environment::{MapEnvironment, set_map_environment, set_map_environment_action};

pub(crate) mod map_ice;
pub use map_ice::{IceChange, MapIceReport, change_map_ice_action};

pub(crate) mod terrain_edit;
pub use terrain_edit::{MapHexChange, set_map_hex_action};

pub(crate) mod map_block;
pub use map_block::add_landing_exclusion_action;
pub(crate) mod map_load;
mod map_objects;
pub use map_load::load_map_action;
pub(crate) mod map_save;
pub use map_save::save_map_action;
mod map_export;
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
pub use static_decoration::{StaticDecoration, StaticDecorationKind, set_static_decoration};

pub(crate) mod map_object_delete;
pub use map_object_delete::{MapObjectKind, delete_map_objects_action};

pub(crate) mod map_list;
pub use map_list::list_map_action;

mod map_link_config;
pub use map_link_config::{MapEntrance, MapLink, set_map_link};

pub(crate) mod map_update_links;
pub use map_update_links::{MapLinkUpdate, update_map_links_action};

pub(crate) mod map_fields;
pub use map_fields::{set_map_field_action, set_map_flag_action};

pub(crate) mod map_check;
pub use map_check::{MapCheck, check_map_action};

pub(crate) mod map_field_report;
pub use map_field_report::{MapField, MapFieldReport, view_map_fields_action};

pub(crate) mod xp_ranking;
pub use xp_ranking::{XpRank, XpRanking, xp_ranking_action};

pub(crate) mod database_save;
pub use database_save::request_database_save;

pub(crate) mod forms_report;
pub use stock_selection::part_forms;

mod operator_settings;
pub use operator_settings::{edit_skill_threshold, edit_weapon_settings};

mod combat_operator;

mod field_bits;
mod field_report;
mod preference_fields;

pub(crate) mod runtime_stats;
pub(crate) mod simulation_pending;
pub use runtime_stats::{RuntimeStats, runtime_stats};

pub(crate) mod scenario_team;
pub use scenario_team::set_team_action;

pub(crate) mod losemit;
pub use losemit::losemit_action;

pub(crate) mod scenario_damage;
pub use scenario_damage::{ScenarioDamage, ScenarioHit, damage_section_action};

pub(crate) mod scenario_packets;
pub use scenario_packets::{ScenarioSalvo, ScenarioSalvoReport, damage_action};

pub(crate) mod weight_report;
pub use weight_report::{WeightEntry, WeightReport, weight_action, weight_report};

pub(crate) mod scenario_position;
pub use scenario_position::{ScenarioPosition, ScenarioPositionReport, set_coordinates_action};

pub(crate) mod battlefield_identity;
pub use battlefield_identity::assign_battlefield_id;

pub(crate) mod scenario_map;
pub use scenario_map::{MapAssignment, reassign_map, remove_map_membership};

pub use scenario_map::{MapIndexReport, set_map_index_action};

pub(crate) mod preferred_identity;
pub use preferred_identity::{
    PreferredId, preferred_id, set_preferred_id, set_preferred_id_action,
};

mod orbital_drop;
mod orbital_drop_combat;
mod orbital_drop_movement;
mod orbital_drop_state;
pub use orbital_drop::{
    DropBreach, DropInterception, DropLanding, DropLandingInput, DropProtection, DropSurface,
    ORBITAL_DROP_ALTITUDE, OrbitalDrop, OrbitalDropStep,
};

pub(crate) mod orbital_drop_launch;
pub use orbital_drop_launch::{OrbitalInsertion, initiate_action as initiate_orbital_drop_action};

mod cockpit_links;
mod hardware_settings;
mod targeting_mode;
pub(crate) mod unit_fields;
mod unit_identity;
pub use unit_fields::{UnitField, UnitFieldReport, set_unit_field_action, view_unit_fields_action};

mod jump_thrust;

pub(crate) mod special_fields;

mod display_name;
pub use display_name::display_name;

mod auxiliary_preferences;
pub use auxiliary_preferences::{set_bth_debug, set_mw_safety};
pub(crate) mod safety;

mod artillery_prediction;
pub use artillery_prediction::{ArtilleryPrediction, predict_artillery_target};

pub(crate) mod snipe;
pub use snipe::snipe_action;

mod damage_field;
mod damage_records;
mod damage_replacement;
pub use damage_field::unit_damage_field;
pub use damage_records::{DamageRecord, parse_damage_field};
pub use damage_replacement::{DamageReplacement, DamageSlot, prepare_damage_field};

mod base_movement_fields;

mod status_fields;

mod motion_fields;

mod registered_unit_defaults;
pub use registered_unit_defaults::{
    ensure_registered_unit_runtime, registered_unit_default_template,
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
pub use component_failure::ComponentFailure;

pub(crate) mod markings;
pub use markings::{set_unit_markings, unit_markings, view_unit_markings};

mod sixth_sense;
pub use sixth_sense::{advance_sixth_sense, advance_sixth_sense_action};

pub(crate) mod turn_clock;

mod periodic_piloting;
pub use periodic_piloting::{
    PeriodicPiloting, advance_periodic_piloting, advance_periodic_piloting_action,
};

pub(crate) use jumping::{launch_dfa_action, launch_jump_action};

mod menu;

mod advantages;
pub use advantages::{AdvantageDefinition, AdvantageKind, BATTLE_ADVANTAGES, advantage_definition};

mod spotter_events;
pub use spotter_events::{SpotterEvents, advance_spotter_links};

mod shot_counters;

mod damage_counters;
mod kill_counters;

mod speed_bonus;

pub(crate) use speed_bonus::SpeedPolicy;

mod roll_history;
mod roll_statistics;
pub use roll_statistics::RollStatistics;

mod tactical;
pub use tactical::{
    MAX_TACTICAL_UNITS, TacticalContact, TacticalIntention, TacticalSighting, TacticalSnapshot,
    TacticalSubmitResult, TacticalUnitSnapshot, observe_tactical, submit_tactical,
};
