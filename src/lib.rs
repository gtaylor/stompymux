//! Independent MUX server components.
pub mod btech;
pub use btech::insert_minefield;
pub use btech::view_map_action as view_battle_map_action;
pub use btech::{
    ArtilleryAim, ArtilleryAimInput, ArtilleryObserver, ArtilleryRange,
    unit_artillery_gunnery_target,
};
pub use btech::{
    ArtilleryCell, ArtilleryEffect, ArtilleryFlight, ArtilleryImpactPattern, ArtilleryMode,
};
pub use btech::{
    ArtilleryHit, ArtilleryImpactReport, advance_artillery_flight, advance_artillery_flight_action,
};
pub use btech::{ArtilleryLaunchReport, LaunchMisload};
pub use btech::{ArtilleryShot, advance_artillery_action, artillery_pending, enqueue_artillery};
pub use btech::{CommandClass, SpecialCommand, SpecialType};
pub use btech::{Ground, MAX_DEPTH, MAX_HEIGHT, Structure, Water, Woods};
pub use btech::{MinePlacement, add_mine_action as add_battle_mine_action};

pub use btech::{
    AimModifiers, AimRules, AmmunitionAdjustment, AmmunitionBin, AmmunitionDraw, AmmunitionHazard,
    AmmunitionMode, AmsReport, ArtemisController, BalanceCause, BalanceReport, BeaconKind,
    BtechState, Character, CharacterInjury, CharacterNotice, CharacterValue, ConsciousnessCheck,
    Contact, ContactEvent, ContactRules, ContactTransition, ContactUpdate, ContactView,
    CriticalDefinition, CriticalLocation, CriticalLoss, DamagePhase, DamageResult, Detection, Dice,
    ElectronicField, ElectronicMode, ElectronicSource, ElectronicSuite, Electronics, Engine,
    Facing, FallRules, FireMode, FreeFall, FreeFallStep, GlancingMode, Gyro, Heat, HeatCheck,
    HeatRates, Hex, HexCoordinate, Hit, HitArc, HitRules, HitTable, ImpactEffect, ImpactReport,
    IndirectAim, JumpCapacity, JumpFlight, JumpOutcome, JumpPath, JumpSample, JumpStep, Light,
    MapAsset, MapPointOfInterest, Mass, Mech, MechFallReport, MechLoadout, MechSalvoReport,
    MechSection, MechShotReport, MechTemplate, MessageTarget, Mobility, Motion, MovementRules,
    NarcReport, Notice, OverheatClock, OverheatReport, OverheatRules, PerceptionAim,
    PersonalEquipment, PersonalLoadout, PilotNotice, PilotingCheck, PlayerConfiguration,
    PodRemoval, PodRow, Point, Position, Posture, Power, Range, RangeBracket, RawMovement,
    RawSectionCode, RawTemplate, RawUnitClass, RecoilReport, Recovery, RecoveryMode,
    ResolvedLoadout, SalvoGroup, SalvoReport, SectionDefinition, SectionExposure,
    SectionExposureReport, SectionState, SensorArc, ShotReport, ShotRules, SkillCategory,
    StackingEntry, StackingInput, StackingRules, Stagger, StaggerHit, StaggerMode, StaggerReport,
    StaggerRules, StandAttempt, StandMode, StandTimer, StoredBattleUnit, StoredMap, Surface,
    SurfaceBreak, System, SystemCritical, TacticalImpact, TacticalPilotInjury, TargetLock,
    TemplateCheck, Terrain, TerrainLos, Torso, UnitConfiguration, UnitDefinition, UnitEdit,
    UnitMut, UnitRef, UnitSignature, Unjam, VehicleHit, VehicleHitRules, VehicleMotiveHit,
    WaterRanges, Weapon, WeaponDamage, WeaponDamageEffects, WeaponDamageKind, WeaponMount,
    WeaponProfile, WeaponRange, WeaponReadiness, WeaponUse, advance_heat as advance_battle_heat,
    advance_jumps as advance_battle_jumps, advance_motion as advance_battle_motion,
    advance_overheat as advance_battle_overheat, advance_recovery as advance_battle_recovery,
    advance_recycle as advance_battle_recycle, advance_stagger as advance_battle_stagger,
    advance_standing as advance_battle_standing, advance_stun as advance_battle_stun,
    advance_target_locks as advance_battle_target_locks, advance_units as advance_battle_units,
    advance_unjamming as advance_battle_unjamming,
    advance_unjamming_action as advance_battle_unjamming_action,
    aim_modifiers as battle_aim_modifiers, apply_damage_phase, assign_pilot as assign_battle_pilot,
    begin_stand as begin_battle_stand, begin_unjam as begin_battle_unjam,
    break_bridge as break_battle_bridge, break_ice as break_battle_ice,
    check_character_consciousness, check_template as check_battle_template,
    contact_observers as battle_contact_observers, create_map as create_battle_map,
    create_unit as create_battle_unit, destroy_unit_critical as destroy_battle_critical,
    displayed_contact as displayed_battle_contact, displayed_contacts as displayed_battle_contacts,
    electronic_field as battle_electronic_field,
    electronic_fields_pending as battle_electronic_fields_pending,
    explode_ammunition as explode_battle_ammunition, flip_arms as flip_battle_arms,
    flood_unit as flood_battle_unit, ground_terrain_los, gunnery_target as battle_gunnery_target,
    injure_character as injure_battle_character,
    injure_tactical_pilot as injure_battle_tactical_pilot, inspect_pods as inspect_battle_pods,
    land_jump as land_battle_jump, launch_jump as launch_battle_jump,
    observer_messages as battle_observer_messages, perception_target as battle_perception_target,
    pilot_aim_modifiers as battle_pilot_aim_modifiers, place_unit as place_battle_unit,
    pod_status as battle_pod_status, prepare_recovery as prepare_battle_recovery,
    read_map as read_battle_map, read_template as read_battle_template,
    refresh_contacts as refresh_battle_contacts,
    refresh_electronic_fields as refresh_battle_electronic_fields, register_empty_battle_unit,
    release_pilot as release_battle_pilot, reload_map as reload_battle_map,
    remove_pod as remove_battle_pod, remove_pod_action as remove_battle_pod_action,
    remove_unit as remove_battle_unit, resolve_electronic_field as resolve_battle_electronic_field,
    resolve_fall as resolve_battle_fall, resolve_impact as resolve_battle_impact,
    resolve_salvo as resolve_battle_salvo, resolve_shot as resolve_battle_shot,
    resolve_stacking as resolve_battle_stacking,
    resolve_tactical_impact as resolve_battle_tactical_impact,
    resolve_tactical_salvo as resolve_battle_tactical_salvo, roll_piloting as roll_battle_piloting,
    roll_unit_dice, rotate_torso as rotate_battle_torso, select_target as select_battle_target,
    set_ams as set_battle_ams, set_auto_fall as set_battle_auto_fall,
    set_character as set_battle_character, set_character_value as set_battle_character_value,
    set_heading as set_battle_heading, set_inarc_ammunition as set_battle_inarc_ammunition,
    set_map_visibility as set_battle_map_visibility, set_rotary as set_battle_rotary,
    set_speed as set_battle_speed, set_unit_signature as set_battle_unit_signature,
    spend_weapon as spend_battle_weapon, stand_target as battle_stand_target,
    start_unit as start_battle_unit, stop_unit as stop_battle_unit, stun_unit as stun_battle_unit,
    toggle_armor_piercing as toggle_battle_armor_piercing, toggle_artemis as toggle_battle_artemis,
    toggle_caseless as toggle_battle_caseless, toggle_cluster as toggle_battle_cluster,
    toggle_electronics as toggle_battle_electronics, toggle_explosive as toggle_battle_explosive,
    toggle_flamer_heat as toggle_battle_flamer_heat, toggle_flechette as toggle_battle_flechette,
    toggle_gatling as toggle_battle_gatling, toggle_hotload as toggle_battle_hotload,
    toggle_incendiary as toggle_battle_incendiary, toggle_lbx as toggle_battle_lbx,
    toggle_narc as toggle_battle_narc, toggle_precision as toggle_battle_precision,
    toggle_rapid as toggle_battle_rapid, toggle_ultra as toggle_battle_ultra,
    unit_elevation as battle_unit_elevation, unit_gunnery_target as battle_unit_gunnery_target,
    unit_piloting_target as battle_unit_piloting_target, unit_range as battle_unit_range,
    unit_terrain_los as battle_unit_terrain_los, update_contact as update_battle_contact,
    visible_contact as visible_battle_contact, visible_contacts as visible_battle_contacts,
    weapon_bears_on as battle_weapon_bears_on,
};
pub mod accounts;
pub use accounts::{Account, Login};
pub mod commands;
pub use commands::{
    Action as CommandAction, CommandContext, CommandDefinition, CommandHandler, CommandInput,
    CommandMatcher, CommandPermissions, CommandRegistry, CommandScope, ExecutionContext,
    InputOrigin, Report as CommandReport, ServerRequest, SwitchDefinition,
};
pub mod config;
pub use config::Config;
mod destruction;
pub mod lua;
pub use lua::sources::Sources as LuaSources;
pub use lua::{AdminRequest as LuaAdminRequest, AppearanceMode, RuntimeMode};
pub use lua::{Scripts, WorldInspection};
pub mod persistence;
pub mod server;
pub use server::{ShutdownRequest, prepare as prepare_server, serve};
pub mod sessions;
pub mod telnet;
pub mod text;
pub use text::{ColorDepth, Document, Palette, RenderOptions};
mod world;
pub use world::{CreationContext, Kind, LinkSlots, Links, Object, ObjectId, SharedMap, World};

pub mod flags;
pub use flags::{Flag, FlagSet};
pub mod movement;
pub mod powers;

pub mod find;
pub mod reports;
pub mod runtime;
pub mod search;
pub use runtime::{EffectBatch, Effects, Outbox, PrivateOutput, SharedWorld};

/// Transactional database checking and semantic repair.
pub mod dbck;

/// Startup-indexed Markdown help.
pub mod help;

/// Channels, communication policy and online paging.
pub mod communication;
pub use communication::Channel;

/// Binary-safe typed object state and administrative operations.
pub mod state;
pub use state::Value as StateValue;

/// Captured Lua scheduling and clock-driven execution primitives.
pub use lua::schedules::{
    Catalog as ScheduleCatalog, Cron, Definition as ScheduleDefinition, Job as ScheduledJob,
    Queue as ScheduleQueue, jitter as schedule_jitter,
};
/// Embedded server entry point with a schedule-only test clock and a chosen heartbeat driver.
pub use server::run_with_schedule_clock;

/// Player-defined command templates and shared macro sets.
pub mod macros;
pub use macros::{MacroEntry, MacroModes, MacroSet, MacroSetId, MacroSlots, PlayerMacros};

/// Shared object and channel lock catalog.
mod locks;
pub use locks::{LOCKS, LockInvocation, LockOutcome, LockType};

/// Live-world Lua suite execution and bounded reports.
pub use lua::testing::{Report as LuaTestReport, Request as LuaTestRequest};

/// Wizard account requests and login-history inspection.
pub mod account_admin;

/// Bounded container and audible-exit message routing.
pub mod notification;

pub mod cleaning;

pub mod controls;
pub mod message_cache;

/// Ordered connection access policies.
pub mod sites;

/// Shared command and list access evaluation.
pub mod access;

pub mod logging;

/// Read-only operational reports and platform resource snapshots.
pub mod operations;

/// Object authority independent of flag mutation and lock evaluation.
pub mod authority;
/// Wall-clock timestamps, separate from monotonic runtime deadlines.
pub mod clock;

pub use btech::{
    Searchlight, SearchlightMode, advance_searchlights as advance_battle_searchlights,
    set_searchlight_mode as set_battle_searchlight_mode,
    toggle_searchlight as toggle_battle_searchlight, unit_illuminated as battle_unit_illuminated,
};

pub use btech::{
    illumination_pending as battle_illumination_pending,
    refresh_illumination as refresh_battle_illumination,
    set_searchlight_warning as set_battle_searchlight_warning,
};

pub use btech::{
    set_ammunition_warning as set_battle_ammunition_warning,
    set_armor_warning as set_battle_armor_warning,
};

pub use btech::set_friendly_fire_safety as set_battle_friendly_fire_safety;

pub use btech::{unit_status as battle_unit_status, weapon_status as battle_weapon_status};

pub use btech::{
    LancePenetration, Leg, PhysicalProfile, PhysicalReport, PhysicalRules,
    kick_profile as battle_kick_profile, resolve_kick as resolve_battle_kick,
};

// Physical arm selection and punch reports share the kick damage domain.
pub use btech::{
    Arm, ArmAttackReport, ArmRejection, ArmSelection, PhysicalAttack,
    punch_profile as battle_punch_profile, resolve_punch as resolve_battle_punch,
};

// Leg trips use shared physical rules and reports.
pub use btech::{resolve_trip as resolve_battle_trip, trip_profile as battle_trip_profile};

pub use btech::{
    ArmAttack, arm_attack_profile as battle_arm_attack_profile,
    resolve_arm_attack as resolve_battle_arm_attack,
};

pub use btech::{
    club_profile as battle_club_profile, grab_club as grab_battle_club,
    resolve_club as resolve_battle_club,
};

pub use btech::{
    ChargeBalance, ChargeProfile, ChargeReport, ChargeRules,
    charge_profile as battle_charge_profile, resolve_charge as resolve_battle_charge,
};

pub use btech::{
    MutualChargeAttempt, MutualChargeReport, resolve_mutual_charge as resolve_battle_mutual_charge,
};

pub use btech::{
    ChargePolicy, ChargeSelection, ChargeState, select_charge as select_battle_charge,
};

pub use btech::{
    DfaBalance, DfaProfile, DfaReport, dfa_profile as battle_dfa_profile,
    resolve_dfa as resolve_battle_dfa,
};

pub use btech::launch_dfa as launch_battle_dfa;

pub use btech::{
    ExperienceAward, ExperienceRules,
    award_character_experience as award_battle_character_experience,
};

pub use btech::{
    BATTLE_SKILLS, SkillDefinition, SkillProgress,
    award_skill_experience as award_battle_skill_experience,
    set_skill_threshold as set_battle_skill_threshold, skill_definition as battle_skill_definition,
    skill_progress as battle_skill_progress, skill_threshold as battle_skill_threshold,
};

pub use btech::clear_character as clear_battle_character;

pub use btech::retain_character_experience as retain_battle_character_experience;

pub use btech::CrewCasualty;

pub use btech::resolve_impact_action as resolve_battle_impact_action;

pub use btech::{
    CharacterPilotInjury, CharacterPilotStatus,
    injure_character_pilot as injure_battle_character_pilot,
};

pub use btech::injure_character_pilot_action as injure_battle_character_pilot_action;
/// Apply and publish vehicle armor damage with atomic character casualty handling.
pub use btech::resolve_vehicle_armor_damage_action as resolve_battle_vehicle_armor_damage_action;
/// Apply and publish a vehicle critical with atomic character casualty handling.
pub use btech::resolve_vehicle_critical_action as resolve_battle_vehicle_critical_action;

pub use btech::flood_unit_action as flood_battle_unit_action;

pub use btech::fall_unit_action as fall_battle_unit_action;

/// Publish upward ice breakout and neighboring casualties atomically.
pub use btech::break_ice_upward_action as break_battle_ice_upward_action;
pub use btech::break_surface_action as break_battle_surface_action;

/// Advance airborne events and publish character casualties atomically.
pub use btech::advance_jumps_action as advance_battle_jumps_action;

/// Resolve crowding and publish character injuries and casualties atomically.
pub use btech::stacking_action as resolve_battle_stacking_action;

/// Advance ground motion and publish character consequences atomically.
pub use btech::advance_motion_action as advance_battle_motion_action;

/// Resolve a physical attack and publish character injuries and crew casualties atomically.
pub use btech::physical_attack_action as resolve_battle_physical_attack_action;

pub use btech::arm_attack_action as resolve_battle_arm_attack_action;
/// Arm sequencing choice and character-aware host action.
pub use btech::{ArmAttackChoice, ArmWeapon};

/// Resolve one charge and publish character injuries, XP and casualties atomically.
pub use btech::charge_action as resolve_battle_charge_action;

/// Resolve both mutual charges with character casualty publication.
pub use btech::mutual_charge_action as resolve_battle_mutual_charge_action;

/// Resolve DFA with atomic character injury, XP and casualty publication.
pub use btech::dfa_action as resolve_battle_dfa_action;

pub use btech::stand_action as begin_battle_stand_action;

pub use btech::stagger_action as advance_battle_stagger_action;

pub use btech::ammunition_explosion_action as explode_battle_ammunition_action;

pub use btech::overheat_action as advance_battle_overheat_action;

pub use btech::salvo_action as resolve_battle_salvo_action;

pub use btech::shot_action as resolve_battle_shot_action;

pub use btech::{GunneryExperienceChance, GunneryExperienceInput};

pub use btech::{
    GunneryExperienceMode, UnitExperience,
    gunnery_experience_eligible as battle_gunnery_experience_eligible,
    set_unit_experience as set_battle_unit_experience,
};

pub use btech::{
    GunneryAwardRequest, GunneryExperienceAward,
    award_classic_gunnery_experience as award_battle_classic_gunnery_experience,
};

pub use btech::BattleValue;

pub use btech::{
    BattleValueExperienceAward, BattleValueExperienceCalculation, BattleValueExperienceInput,
    award_battle_value_gunnery_experience, pilot_battle_value_modifier,
};

pub use btech::{ShotExperienceAward, award_gunnery_experience as award_battle_gunnery_experience};

pub use btech::{DiagnosticMessage, DiagnosticTopic};

pub use btech::land_action as land_battle_jump_action;

pub use btech::{
    SignatureState, SignatureTransition, advance_stealth as advance_battle_stealth,
    toggle_stealth as toggle_battle_stealth,
};

pub use btech::{
    advance_null_signature as advance_battle_null_signature,
    toggle_null_signature as toggle_battle_null_signature,
};

pub use btech::clear_recent_fire as clear_battle_recent_fire;

/// Automatic perception: sensor band, sight, active probes and radar.
pub use btech::{
    AUTOMATIC_DETECTION_RANGE, AcquisitionRules, ActiveProbe, DEFAULT_SENSOR_RANGE,
    DetectionChannel, HIDDEN_DETECTION_RANGE, MapFlag, MapPerceptionFlag, Perception,
    PerceptionProfile, PerceptionReport, PerceptionStatus, ProbeProfile, RADAR_RANGE, RadarProfile,
    RadarTarget, SensorRange, configure_perception as configure_battle_perception,
    hex_perception as battle_hex_perception, perceive as battle_perceive,
    perception_factor as battle_perception_factor, perception_profile as battle_perception_profile,
    perception_report as battle_perception_report, set_map_perception as set_battle_map_perception,
};

pub use btech::{
    TagState, advance_tags as advance_battle_tags, select_tag as select_battle_tag,
    tagged_by as battle_tagged_by,
};

pub use btech::toggle_semiguided as toggle_battle_semiguided;

pub use btech::{
    SpotterTarget, select_spotter as select_battle_spotter, spotter_target as battle_spotter_target,
};

pub use btech::hex_visible as battle_hex_visible;

pub use btech::{
    HexLock, HexTargetMode, TargetSelection, select_hex_target as select_battle_hex_target,
};

pub use btech::hex_occupant as battle_hex_occupant;

pub use btech::map_unit_order as battle_map_unit_order;

pub use btech::{
    HexAimModifiers, hex_aim_modifiers as battle_hex_aim_modifiers,
    pilot_hex_aim_modifiers as battle_pilot_hex_aim_modifiers,
};

pub use btech::{WoodlandClearing, WoodlandEffect, WoodlandIntent, resolve_woodland_effect};

pub use btech::{WoodlandChange, apply_woodland_clearing};

pub use btech::{
    Decoration, DecorationKind, advance_map_smoke, map_smoke_pending, set_map_decoration,
};

pub use btech::set_map_wind;

pub use btech::{advance_map_fire, map_fire_pending};

pub use btech::{WoodlandAttack, WoodlandImpact, resolve_woodland_attack};

pub use btech::{HexShotReport, resolve_hex_shot as resolve_battle_hex_shot};

pub use btech::FireReport;

/// Weapon-triggered ice and bridge fracture report.
pub use btech::SurfaceWeaponImpact;

/// Interior-map construction state and its checked configuration operation.
pub use btech::{BuildingState, set_building_state};

/// Building entrance identity and checked configuration.
pub use btech::{BuildingEntrance, set_building_entrance};

/// Building packet consequences and committed repair advancement.
pub use btech::{BuildingImpact, advance_building_repairs, building_repair_pending};

/// Ordered minefield definitions and checked administrative configuration.
pub use btech::{MineKind, Minefield, set_minefield};

/// Read-only mine coverage and event selection.
pub use btech::{MineActivation, MineResponse, MineTriggerReason, mine_activations};

/// Conventional mine blasts and their character-aware host transaction.
pub use btech::{
    BlastImpact, MineBlastHit, MineBlastReport, resolve_mine_blast, resolve_mine_blast_action,
};

/// Persisted inferno application and committed duration advancement.
pub use btech::{advance_inferno_burns, apply_inferno_burn, extinguish_inferno_in_water};

/// Atomic inferno missile exposure and immediate immersion effects.
pub use btech::{InfernoHit, resolve_inferno_hit};

/// Select inferno missile ammunition.
pub use btech::toggle_inferno as toggle_battle_inferno;

/// Physical mine activation and its applied effects.
pub use btech::{MineEventReport, activate_mines};

/// Frequency-matched command mine resolution and host publication.
pub use btech::{CommandMineReport, detonate_command_mines, detonate_command_mines_action};

/// Owned radio channel settings and equipment-derived limits.
pub use btech::{
    RadioCapabilities, RadioChannel, RadioMode, set_radio_frequency, set_radio_mode,
    set_radio_title,
};

/// Digital radio routing and formatted receptions.
pub use btech::{DigitalRadioReport, RadioReception, resolve_digital_radio};

/// Analog radio delivery with saved receiver interference dice.
pub use btech::{AnalogRadioReport, resolve_analog_radio};

/// Atomic radio transmission and command-mine publication.
pub use btech::{RadioDelivery, RadioTransmission, send_radio_action};

/// Frequency settings with transactional administrative diagnostics.
pub use btech::set_radio_frequency_action;

/// Reception-gated communication skill experience.
pub use btech::award_radio_experience;

/// Trusted observer-role assignment.
pub use btech::{set_observer as set_battle_observer, unit_observer as battle_unit_observer};

/// Targeted line-of-sight radio resolution and publication.
pub use btech::{TargetedRadioReport, resolve_targeted_radio, send_targeted_radio_action};

/// Detailed unit scans and derived computer sensor ranges.
pub use btech::{
    SensorRanges, scan_unit as scan_battle_unit, scan_unit_action as scan_battle_unit_action,
};

/// Coordinate-based detailed inspection of acquired unit occupants.
pub use btech::scan_hex_unit_action as scan_battle_hex_unit_action;

/// Structure inspection and transactional cockpit publication.
pub use btech::{
    BuildingScan, scan_building as scan_battle_building,
    scan_building_action as scan_battle_building_action,
};

/// Combined structure and minefield inspection.
pub use btech::{
    HexScan, MineScan, scan_hex_action as scan_battle_hex_action, scan_mines as scan_battle_mines,
};

/// Scanning the currently selected unit or coordinate target.
pub use btech::{SelectedScan, scan_selected_action as scan_battle_selected_action};

/// Silent summary inspection of visible units.
pub use btech::report_unit as report_battle_unit;

/// Tactical and long-range display centering.
pub use btech::{
    ViewCenter, ViewKind, ViewPosition, parse_view_center as parse_battle_view_center,
    resolve_view_center as resolve_battle_view_center,
};

/// Bounded display dimensions and map-edge clipping.
pub use btech::{ViewDimensions, Viewport, resolve_viewport as resolve_battle_viewport};

/// Long-range terrain, elevation and visible-unit displays.
pub use btech::{LongRangeMap, LongRangeMode, long_range_map as battle_long_range_map};

/// Current terrain illumination without contact acquisition.
pub use btech::hex_illuminated as battle_hex_illuminated;

pub use btech::{TacticalMap, tactical_map as battle_tactical_map};

pub use btech::{
    LandingExclusion, LandingSuitability, set_landing_exclusion as set_battle_landing_exclusion,
};

pub use btech::{HexCenterReport, find_center as find_battle_hex_center};

pub use btech::{NavigationReport, navigate as battle_navigate};

pub use btech::{
    set_view_dimensions as set_battle_view_dimensions, view_dimensions as battle_view_dimensions,
};

pub use btech::{
    BuildingContactMode, ContactPreferences, PlayerPreferences,
    contact_preferences as battle_contact_preferences,
    filtered_contacts as filtered_battle_contacts,
    set_contact_preferences as set_battle_contact_preferences,
};

pub use btech::{ContactOptions, parse_contact_options as parse_battle_contact_options};

pub use btech::{BuildingContact, building_contacts as battle_building_contacts};

pub use btech::{BriefReport, BriefSettings, brief as battle_brief};

pub use btech::parse_contact_options_for_display as parse_battle_contact_options_for_display;

pub use btech::ContactArc;

pub use btech::set_autocon_shutdown as set_battle_autocon_shutdown;

pub use btech::hex_detection as battle_hex_detection;

pub use btech::{
    LateralMode, LateralState, lateral as battle_lateral, set_lateral as set_battle_lateral,
};

pub use btech::{
    BootleggerReport, bootlegger as battle_bootlegger,
    bootlegger_modifier as battle_bootlegger_modifier,
};

pub use btech::{EtaReport, eta as battle_eta, eta_action as battle_eta_action};

pub use btech::{BearingReport, bearing as battle_bearing};

pub use btech::{RangeReport, range_display as battle_range_report};

pub use btech::{VectorReport, vector_display as battle_vector_report};

pub use btech::motion_readout as battle_motion_readout;

pub use btech::{
    Dump, DumpIgnition, DumpSelection, advance_dumping as advance_battle_dumping,
    begin_dump as begin_battle_dump, dump as battle_dump,
};

pub use btech::{
    BoosterState, advance_boosters_action as advance_battle_boosters_action, masc as battle_masc,
    supercharger as battle_supercharger, toggle_masc as toggle_battle_masc,
    toggle_supercharger as toggle_battle_supercharger,
};

/// Installed and functional command-network computers.
pub use btech::C3Hardware;

/// C3i membership controls and inspection.
pub use btech::command_network::{
    c3i as battle_c3i, join_leave as join_leave_battle_c3i, members as battle_c3i_members,
};

/// Network-provided range assistance in conventional aim reports.
pub use btech::NetworkRange;

/// C3i network communication and transactional cockpit delivery.
pub use btech::network_message::{
    message as prepare_battle_c3i_message, send as battle_c3i_message,
};

/// Read-only command-network peer status and private cockpit display.
pub use btech::network_status::{
    NetworkStatusReport, NetworkStatusRow, c3_status as battle_c3_status,
    status as battle_c3i_status,
};

/// Automatic command-network formation and its per-unit opt-out.
pub use btech::{
    NetworkAutomation, reconcile_command_networks as reconcile_battle_command_networks,
};

/// Classic C3 membership using the shared command-network model.
pub use btech::command_network::{
    CommandNetwork, NetworkRequest, c3 as battle_c3, c3_members as battle_c3_members,
    join_leave_c3 as join_leave_battle_c3, request_for as request_battle_network,
};

/// Classic C3 communication through the shared network delivery engine.
pub use btech::network_message::{
    c3_message as prepare_battle_c3_message, send_c3 as battle_c3_message,
};

/// Receiver-owned analog frequency search results.
pub use btech::FrequencyScan;

/// Chassis-specific anatomy and template section naming.
pub use btech::MechChassis;

/// Ground-vehicle asset definitions, independent of BattleMech simulation admission.
pub use btech::{
    VehicleMovement, VehicleSection, VehicleTemplate,
    read_vehicle_template as read_battle_vehicle_template,
};

/// Catalogue-resolved vehicle slots, distinct from live vehicle construction.
pub use btech::{VehicleCriticalLocation, VehicleLoadout};

/// Ground-vehicle engine rating, suspension and fixed-point mass diagnostics.
pub use btech::{VehicleEngine, VehiclePowerplant};

/// Intact ground-vehicle component and ammunition mass diagnostics.
pub use btech::VehicleMass;

/// Owned ground-vehicle protection and ammunition state.
pub use btech::Vehicle;

/// Register owned ground-vehicle state on an unused object.
pub use btech::create_vehicle as create_battle_vehicle;

/// Apply material damage to an owned ground vehicle in a world candidate.
pub use btech::damage_vehicle_phase as damage_battle_vehicle_phase;

/// Explicit Mech/ground-vehicle construction asset dispatch.
pub use btech::{
    ArtilleryAiming, MountArcs, StealthRange, TerrainIgnition, VehicleMountArcs,
    VehicleTemplateMotion, VtolHitLocation, WeaponSalvo,
};
pub use btech::{UnitTemplate, UnitTemplateExt, read_unit_template as read_battle_unit_template};

/// World and pilot inputs for intact ground-vehicle motion proposals.
pub use btech::VehicleMotionRules;

/// Apply a vehicle motive hit within the owning world transaction.
pub use btech::damage_vehicle_motive as damage_battle_vehicle_motive;

/// Ground-vehicle turret facing query and control.
pub use btech::{set_turret as set_battle_turret, turret_readout as battle_turret_readout};

/// Apply persistent vehicle turret-lock damage within a world transaction.
pub use btech::lock_vehicle_turret as lock_battle_vehicle_turret;

/// Reserve a vehicle firing cycle; the caller must apply attack effects.
pub use btech::{VehicleWeaponUse, reserve_vehicle_weapon as reserve_battle_vehicle_weapon};

/// Read target movement contributions for Mechs and ground vehicles.
pub use btech::unit_target_movement_modifier as battle_unit_target_movement_modifier;

/// Toggle Stinger ammunition on an intact, recycled indirect missile launcher.
pub use btech::toggle_stinger as toggle_battle_stinger;

/// Destroy an installed vehicle equipment slot inside an enclosing damage transaction.
pub use btech::destroy_vehicle_critical as destroy_battle_vehicle_critical;

/// Choose a vehicle weapon critical with saved unit dice; the caller applies its damage effects.
pub use btech::select_vehicle_weapon_critical as select_battle_vehicle_weapon_critical;

/// Ground-vehicle critical table selection and unapplied consequence reports.
pub use btech::{
    VehicleCriticalEffect, VehicleCriticalReport, VehicleCriticalRules, VehicleCriticalTable,
    roll_vehicle_critical as roll_battle_vehicle_critical,
};

/// Vehicle driver, sensor and stabilizer damage with saved control penalties.
pub use btech::{VehicleControlHit, damage_vehicle_controls as damage_battle_vehicle_controls};

pub use btech::{
    EquipmentFailure, VehicleWeaponJam, jam_vehicle_weapon as jam_battle_vehicle_weapon,
};

pub use btech::{
    begin_vehicle_turret_repair as begin_battle_turret_repair,
    jam_vehicle_turret as jam_battle_vehicle_turret,
};

pub use btech::{
    VehicleCriticalResolution, resolve_vehicle_critical as resolve_battle_vehicle_critical,
};

pub use btech::{VehicleMainWeaponJam, jam_vehicle_main_weapon as jam_battle_vehicle_main_weapon};

pub use btech::{
    VehicleAmmunitionCascade,
    discharge_vehicle_ammunition_cascade as discharge_battle_vehicle_ammunition_cascade,
};

pub use btech::{
    VehicleInternalDamage,
    resolve_vehicle_internal_damage as resolve_battle_vehicle_internal_damage,
};

pub use btech::VehicleExplosion;

pub use btech::{
    DamageClass, VehicleArmorDamage, VehicleArmorHit,
    resolve_vehicle_armor_damage as resolve_battle_vehicle_armor_damage,
};

pub use btech::{
    VehicleImpact, VehicleImpactRules, resolve_vehicle_impact as resolve_battle_vehicle_impact,
};

pub use btech::check_vehicle_shot as check_battle_vehicle_shot;

pub use btech::{
    VehicleLaunch, VehicleLaunchRequest, launch_vehicle_weapon as launch_battle_vehicle_weapon,
};

pub use btech::{
    VehicleSalvoGroup, VehicleSalvoReport, VehicleSalvoRequest,
    resolve_vehicle_salvo as resolve_battle_vehicle_salvo,
};

pub use btech::{
    TargetSalvo, VehicleShotReport, VehicleShotRules, fire_vehicle_shot as fire_battle_vehicle_shot,
};

/// Typed beacon attachment section for mixed construction shot reports.
pub use btech::UnitSection;

/// Start a saved vehicle crew action to remove attached iNarc effects.
pub use btech::{
    begin_pod_removal as begin_battle_pod_removal,
    begin_pod_removal_action as begin_battle_pod_removal_action,
};

pub use btech::{
    VehicleFireTick, VehicleHeatExposure, VehicleInfernoHit,
    advance_vehicle_fires as advance_battle_vehicle_fires,
    advance_vehicle_fires_action as advance_battle_vehicle_fires_action,
    begin_vehicle_extinguishing as begin_battle_vehicle_extinguishing,
    begin_vehicle_extinguishing_action as begin_battle_vehicle_extinguishing_action,
    resolve_vehicle_fire_exposure_action as resolve_battle_vehicle_fire_exposure_action,
    resolve_vehicle_heat_exposure as resolve_battle_vehicle_heat_exposure,
    resolve_vehicle_heat_exposure_action as resolve_battle_vehicle_heat_exposure_action,
    resolve_vehicle_inferno_hit as resolve_battle_vehicle_inferno_hit,
    resolve_vehicle_inferno_hit_action as resolve_battle_vehicle_inferno_hit_action,
};

pub use btech::{
    VehicleFireEffects, VehicleFireExposure,
    resolve_vehicle_fire_exposure as resolve_battle_vehicle_fire_exposure,
};

pub use btech::advance_vtol_environment as advance_battle_vtol_environment;
pub use btech::resolve_vtol_crash as resolve_battle_vtol_crash;
pub use btech::{
    ByChassis, UnitFallReport, UnitShotReport, fire_unit_shot as fire_battle_unit_shot,
    resolve_unit_fall as resolve_battle_unit_fall,
};
pub use btech::{
    FallReport, MechFallFeedback, VehicleFallFeedback, VehicleFallReport,
    resolve_vehicle_fall as resolve_battle_vehicle_fall,
};
pub use btech::{VehicleDescentEvent, advance_vtol_fall as advance_battle_vtol_fall};
pub use btech::{
    begin_vtol_takeoff as begin_battle_vtol_takeoff,
    set_vtol_vertical_speed as set_battle_vtol_vertical_speed,
    vtol_vertical_readout as battle_vtol_vertical_readout,
};

/// Character-capable vehicle falls with transactional casualty publication.
pub use btech::vehicle_fall_action as resolve_battle_vehicle_fall_action;

pub use btech::{RotorHit, VtolHit};

pub use btech::RotorDamage;

pub use btech::{
    VtolFuel, VtolFuelStatus, VtolFuelUse, set_vtol_fuel as set_battle_vtol_fuel,
    vtol_fuel_status as battle_vtol_fuel_status,
};

pub use btech::{VtolFlight, VtolFlightPhase, VtolTakeoff};

pub use btech::VtolLanding;

pub use btech::{VtolMotionStep, VtolSurfaceContact};

pub use btech::VtolPath;

pub use btech::VtolEnvironment;

pub use btech::{
    BuildingEntryPoint, building_entry_destination as battle_building_entry_destination,
    building_exit_destination as battle_building_exit_destination,
    set_building_entry_point as set_battle_building_entry_point,
    set_building_exit as set_battle_building_exit,
};

pub use btech::transfer_unit as transfer_battle_unit;

pub use btech::{
    BuildingEntry, advance_building_entry as advance_battle_building_entry,
    begin_building_entry as begin_battle_building_entry, building_entry as battle_building_entry,
    building_entry_destination_for_unit as battle_building_entry_destination_for_unit,
    building_entry_lock_allows as battle_building_entry_lock_allows,
    clear_building_entry as clear_battle_building_entry,
};

pub use btech::{
    BuildingArrival, advance_building_entries_action as advance_battle_building_entries_action,
    begin_building_entry_action as begin_battle_building_entry_action,
    building_entries_pending as battle_building_entries_pending,
    publish_building_arrivals as publish_battle_building_arrivals,
};

pub use btech::{
    building_exit_for_unit as battle_building_exit_for_unit, exit_building as exit_battle_building,
};

pub use btech::exit_building_action as exit_battle_building_action;

pub use btech::set_tow as set_battle_tow;

pub use btech::{UnitLoad, unit_load as battle_unit_load};

pub use btech::throttle_maximum as battle_throttle_maximum;

/// World-aware transfer speed derived from live mass and external tow load.
pub use btech::unit_effective_maximum_speed as battle_effective_maximum_speed;

/// Battle value using live external load and configured myomer towing assistance.
pub use btech::unit_battle_value as battle_unit_value;

/// Shared pickup eligibility and persisted scenario towing permission.
pub use btech::{
    pickup_admission as battle_pickup_admission, set_towable as set_battle_towable,
    unit_towable as battle_unit_towable,
};

/// Prepare an admitted tow target inside an enclosing pickup transaction.
pub use btech::prepare_pickup as prepare_battle_pickup;

/// Shared ground-vehicle and rotorcraft forced-descent transitions.
pub use btech::{
    advance_vehicle_descent as advance_battle_vehicle_descent,
    begin_vehicle_descent as begin_battle_vehicle_descent,
};

/// Release prepared tow ownership and settle the target or start forced descent.
pub use btech::release_tow as release_battle_tow;

/// Shared pickup material and terrain transaction.
pub use btech::{PickupReport, pickup_unit as pickup_battle_unit};

/// Atomic pickup/dropoff with host publication and casualty handling.
pub use btech::tow_action as battle_tow_action;

/// Ground-vehicle cover preparation and saved digging state.
pub use btech::{DigState, dig_unit as dig_battle_unit};

/// Begin vehicle cover with atomic host notification.
pub use btech::dig_action as battle_dig_action;

pub use btech::{
    HullDownState, hull_down_action as battle_hull_down_action,
    set_hull_down as set_battle_hull_down,
};

pub use btech::{set_fortified as set_battle_fortified, unit_fortified as battle_unit_fortified};

pub use btech::unit_altitude as battle_unit_altitude;

pub use btech::unit_radio_capabilities;

pub use btech::{TicEdit, Tics, edit_battle_tic, tic};

pub use btech::{FireTarget, TicShot, fire_battle_tics};

pub use btech::{HeatCutoff, toggle_battle_heat_cutoff};

pub use btech::set_battle_ammunition_section;

pub use btech::{
    advance_battle_automatic_turrets, automatic_turrets_pending, toggle_battle_automatic_turret,
};

pub use btech::{ReactorBlastHit, ReactorExplosion, reactor_explosion_action};

pub use btech::{
    SelfDestruct, SelfDestructOutcome, advance_battle_self_destructs_action, self_destruct_action,
    self_destructs_pending,
};

pub use btech::set_battle_self_destruct_safe;

pub use btech::{
    advance_battle_reactor_windows, configure_battle_reactor_policy, reactor_windows_pending,
};

pub use btech::{advance_battle_wrecks_action, wrecks_pending};

pub use btech::{ProneReport, prone_action};

pub use btech::{advance_battle_hiding, begin_battle_hiding, hiding_pending};

/// Shared operator firing restriction and trusted scenario mutation.
pub use btech::{set_battle_weapons_hold, weapons_hold};

/// Trusted scenario visibility overrides shared by Mechs and vehicles.
pub use btech::{Visibility, set_battle_visibility, visibility};

/// Trusted combat immunity shared by every supported chassis.
pub use btech::{combat_safe, set_battle_combat_safe};

pub use btech::{SwarmHop, SwarmReport};

pub use btech::toggle_swarm as toggle_battle_swarm;

pub use btech::toggle_missile_rounds as toggle_battle_missile_rounds;

pub use btech::toggle_thunder as toggle_battle_thunder;

pub use btech::{THUNDER_MAXIMUM_STRENGTH, ThunderField, ThunderReport};

pub use btech::{SightAim, SightReport};

pub use btech::{
    EquipmentCondition, WeaponDiagnostic, WeaponSpecification,
    weapon_diagnostic_text as battle_weapon_diagnostic_text,
    weapon_diagnostics as battle_weapon_diagnostics,
    weapon_specification_text as battle_weapon_specification_text,
    weapon_specifications as battle_weapon_specifications,
};

pub use btech::{
    CriticalInspection, CriticalReport, critical_report as battle_critical_report,
    critical_status as battle_critical_status,
};

pub use btech::{
    AimSelection, aimed_section as battle_aimed_section,
    set_aimed_section as set_battle_aimed_section,
};

pub use btech::{
    WeaponSettings, WeaponValues, set_weapon_battle_value as set_battle_weapon_battle_value,
    set_weapon_recycle as set_battle_weapon_recycle,
};

pub use btech::{
    InventoryEntry, inventory as battle_inventory,
    set_inventory_quantity as set_battle_inventory_quantity,
    set_inventory_quantity_action as set_battle_inventory_quantity_action,
};

pub use btech::{
    AMMUNITION_PART_OFFSET, PART_ID_LIMIT, Part, PartKind, WEAPON_PART_IDS,
    inventory_mass as battle_inventory_mass, set_inventory_named as set_battle_inventory_named,
};

pub use btech::{
    CargoTransferPoint, check_cargo_transfer_point as check_battle_cargo_transfer_point,
    set_cargo_transfer_point as set_battle_cargo_transfer_point,
};

pub use btech::{
    CargoOperation, CargoRow, cargo_manifest as battle_cargo_manifest,
    transfer_cargo as transfer_battle_cargo, transfer_cargo_action as transfer_battle_cargo_action,
};

pub use btech::{InventoryChange, change_inventory_action as change_battle_inventory_action};

pub use btech::add_stores_action as add_battle_stores_action;

pub use btech::disable_gauss_weapon;

pub use btech::toggle_atm_ammunition;
pub use btech::toggle_mml_ammunition;

pub use btech::{
    InventoryCleanup, clean_inventory as clean_battle_inventory,
    clean_inventory_action as clean_battle_inventory_action,
};

pub use btech::{
    MapEnvironment, set_map_environment as set_battle_map_environment,
    set_map_environment_action as set_battle_map_environment_action,
};

/// Change the persisted cloud boundary for shared optical sensors.
pub use btech::set_map_cloud_base as set_battle_map_cloud_base;

pub use btech::{IceChange, MapIceReport, change_map_ice_action as change_battle_map_ice_action};

pub use btech::{MapHexChange, set_map_hex_action as set_battle_map_hex_action};

pub use btech::WoodsAbsorption;
pub use btech::add_landing_exclusion_action as add_battle_landing_exclusion_action;
pub use btech::clear_map_units_action as clear_battle_map_units_action;
pub use btech::emit_map_action as emit_battle_map_action;
pub use btech::load_map_action as load_battle_map_action;
pub use btech::resize_map_action as resize_battle_map_action;
pub use btech::save_map_action as save_battle_map_action;
pub use runtime::MapAssetWrite;

pub use btech::stop_unit_action as stop_battle_unit_action;

pub use btech::add_map_decoration_action as add_battle_map_decoration_action;

pub use btech::{BuildingExit, set_building_return_link as set_battle_building_return_link};

pub use btech::StaticDecoration;

pub use btech::StaticDecorationKind;
pub use btech::set_static_decoration as set_battle_static_decoration;

pub use btech::{MapObjectKind, delete_map_objects_action as delete_battle_map_objects_action};

pub use btech::list_map_action as list_battle_map_action;

pub use btech::{MapEntrance, MapLink, set_map_link as set_battle_map_link};

pub use btech::{MapLinkUpdate, update_map_links_action as update_battle_map_links_action};

pub use btech::set_map_field_action as set_battle_map_field_action;
pub use btech::set_map_flag_action as set_battle_map_flag_action;

pub use btech::{MapCheck, check_map_action as check_battle_map_action};

pub use btech::{
    MapField, MapFieldReport, view_map_fields_action as view_battle_map_fields_action,
};

pub use btech::{XpRank, XpRanking, xp_ranking_action as battle_xp_ranking_action};

pub use btech::request_database_save as request_battle_database_save;

pub use btech::{PartForm, part_forms as battle_part_forms};

pub use btech::{
    edit_skill_threshold as edit_battle_skill_threshold,
    edit_weapon_settings as edit_battle_weapon_settings,
};

pub use btech::{RuntimeStats, runtime_stats as battle_runtime_stats};

pub use btech::set_team_action as set_battle_team_action;

pub use btech::losemit_action as battle_losemit_action;

pub use btech::{
    ScenarioDamage, ScenarioHit, damage_section_action as battle_damage_section_action,
};

pub use btech::{ScenarioSalvo, ScenarioSalvoReport, damage_action as battle_damage_action};

pub use btech::{
    WeightEntry, WeightReport, weight_action as battle_weight_action,
    weight_report as battle_weight_report,
};

pub use btech::{
    ScenarioPosition, ScenarioPositionReport,
    set_coordinates_action as set_battle_coordinates_action,
};

pub use btech::assign_battlefield_id;

pub use btech::{MapAssignment, reassign_map as reassign_battle_map};

pub use btech::remove_map_membership as remove_battle_map_membership;

pub use btech::{MapIndexReport, set_map_index_action as set_battle_map_index_action};

pub use btech::{
    PreferredId, preferred_id as battle_preferred_id, set_preferred_id as set_battle_preferred_id,
    set_preferred_id_action as set_battle_preferred_id_action,
};

pub use btech::{
    DropBreach, DropInterception, DropLanding, DropLandingInput, DropProtection, DropSurface,
    ORBITAL_DROP_ALTITUDE, OrbitalDrop, OrbitalDropStep,
};

pub use btech::{
    OrbitalInsertion, initiate_orbital_drop_action as initiate_battle_orbital_drop_action,
};

pub use btech::{
    UnitField, UnitFieldReport, view_unit_fields_action as view_battle_unit_fields_action,
};

pub use btech::set_unit_field_action as set_battle_unit_field_action;

pub use btech::display_name as battle_display_name;

pub use btech::set_bth_debug as set_battle_bth_debug;

pub use btech::set_mw_safety as set_battle_mw_safety;

pub use btech::{
    GroundMotionProposal, propose_mech_ground_motion as propose_battle_mech_ground_motion,
};

pub use btech::{ArtilleryPrediction, predict_artillery_target as predict_battle_artillery_target};

pub use btech::snipe_action as battle_snipe_action;

pub use btech::unit_damage_field as battle_unit_damage_field;
pub use btech::{DamageRecord, parse_damage_field as parse_battle_damage_field};

pub use btech::{
    DamageReplacement, DamageSlot, prepare_damage_field as prepare_battle_damage_field,
};

pub use btech::set_weapon_failure as set_battle_weapon_failure;

pub use btech::ComponentFailure;

pub use btech::{
    set_unit_markings as set_battle_unit_markings, unit_markings as battle_unit_markings,
    view_unit_markings as view_battle_unit_markings,
};

pub use btech::{
    advance_sixth_sense as advance_battle_sixth_sense,
    advance_sixth_sense_action as advance_battle_sixth_sense_action,
};

pub use btech::{
    PeriodicPiloting, advance_periodic_piloting as advance_battle_periodic_piloting,
    advance_periodic_piloting_action as advance_battle_periodic_piloting_action,
};

/// Typed advantage names and interpretation, shared with gameplay and Lua inspection.
pub use btech::{
    AdvantageDefinition, AdvantageKind, BATTLE_ADVANTAGES,
    advantage_definition as battle_advantage_definition,
};

/// Shared durable forward-observer link clocks.
pub use btech::{SpotterEvents, advance_spotter_links as advance_battle_spotter_links};

pub use btech::RollStatistics;

pub use btech::EventTelemetry;

pub use btech::{
    MAX_TACTICAL_UNITS, TacticalContact, TacticalIntention, TacticalSighting, TacticalSnapshot,
    TacticalSubmitResult, TacticalUnitSnapshot, observe_tactical, submit_tactical,
};

pub use server::{
    HeartbeatDriver, HeartbeatHarness, HeartbeatMetrics, HeartbeatTrigger, RuntimeProgress,
};

/// Isolated production-heartbeat performance diagnostics for ground controllers.
pub use btech::autopilot::benchmark::{
    BenchmarkOptions as AutopilotBenchmarkOptions, BenchmarkReport as AutopilotBenchmarkReport,
    BenchmarkResult as AutopilotBenchmarkResult, BenchmarkScenario as AutopilotBenchmarkScenario,
    run as run_autopilot_benchmark,
};
pub use btech::autopilot::runtime::AutopilotRuntimeMetrics;

pub use btech::autopilot::diagnostics::{
    AutopilotDiagnostics, CombatSample as AutopilotCombatSample,
};

// Deterministic combat movement measurement surface.
pub use btech::autopilot::encounters::{
    EncounterResult as AutopilotEncounterResult, run as run_autopilot_encounters,
    run_policy as run_autopilot_encounters_policy,
};

pub use btech::autopilot::adversarial::{
    ParticipantResult as AutopilotAdversarialParticipantResult,
    SCENARIOS as AUTOPILOT_ADVERSARIAL_SCENARIOS, run as run_autopilot_adversarial,
    run_policy as run_autopilot_adversarial_policy,
};

pub use btech::autopilot::interception::PursuitPolicy as AutopilotPursuitPolicy;
pub use btech::autopilot::pursuit_encounters::{
    EXTENDED_SCENARIOS as AUTOPILOT_PURSUIT_EXTENDED_SCENARIOS, Episode as AutopilotPursuitEpisode,
    PursuitResult as AutopilotPursuitResult, SCENARIOS as AUTOPILOT_PURSUIT_SCENARIOS,
    run as run_autopilot_pursuit,
};
