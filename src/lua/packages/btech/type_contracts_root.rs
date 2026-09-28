//! LuaLS contract blocks for the btech root surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00000
//|---BattleTech inspection and supported tactical control APIs implemented by the Rust runtime.
//|---Results are detached values, available only inside game callbacks.
// lua-types-end

// lua-types-begin btech 00001
//|---@alias BattleSectionName "LeftArm"|"RightArm"|"LeftTorso"|"RightTorso"|"CenterTorso"|"LeftLeg"|"RightLeg"|"Head"
// lua-types-end

// lua-types-begin btech 00002
//|---@class BattleNotice
//|---@field unit integer Recipient unit dbref.
//|---@field text string Cockpit message text.
// lua-types-end

// lua-types-begin btech 00003
//|---@class BattleCriticalDefinition
//|---@field equipment string Unresolved asset equipment name.
//|---@field data string Unresolved asset data token.
//|---@field modes string[] Unresolved asset mode names.
//|---@field brand integer|nil Optional asset brand.
// lua-types-end

// lua-types-begin btech 00004
//|---@class BattleSectionDefinition
//|---@field armor integer
//|---@field internal integer
//|---@field rear integer
//|---@field criticals table<integer, BattleCriticalDefinition> Zero-based slot positions.
//|---@field configuration string|nil
// lua-types-end

// lua-types-begin btech 00005
//|---@class BattleTemplate
//|---@field name string
//|---@field reference string
//|---@field tons integer
//|---@field max_speed number
//|---@field jump_speed number
//|---@field heat_sinks integer Cooling capacity; double sinks are already counted twice.
//|---@field sections table<BattleSectionName, BattleSectionDefinition>
//|---@field attributes table<string, string> Unit-level source fields; not validated simulation capabilities.
// lua-types-end

// lua-types-begin btech 00006
//|---@class BattleMapAssetSummary
//|---@field width integer
//|---@field height integer
//|---@field gravity integer
//|---@field temperature integer
//|---@field flags integer
// lua-types-end

// lua-types-begin btech 00007
//|---@class BattleHex
//|---@field terrain string Snake_case terrain name.
//|---@field elevation integer Magnitude from 0 through 9; water and ice represent depth.
// lua-types-end

// lua-types-begin btech 00008
//|---@class StoredBattleMap
//|---@field cargo_transfer_point BattleCargoTransferPoint|nil Saved cargo location and hint policy.
//|---@field wrapping boolean Opposite-edge wrapping is enabled.
//|---@field linked_markers table<integer, {coordinate: BattleHexCoordinate, object: integer, data_char: integer, data_short: integer, data_int: integer}> Complete authored linked marker records.
//|---@field building_exits table<integer, {coordinate: BattleHexCoordinate, destination: integer, data_char: integer, data_short: integer, data_int: integer}> Return-link slots; coordinates are selection metadata.
//|---@field name string
//|---@field width integer
//|---@field height integer
//|---@field gravity integer
//|---@field temperature integer
// lua-types-end

// lua-types-begin btech 00009
//|---@field flags integer
//|---@field light integer 0 night, 1 twilight, 2 day
//|---@field visibility integer Weather range in hexes
//|---@field sensor_flags integer Disabled perception channels: sensor band bit 0 (1), radar bit 5 (32), probes bit 6 (64).
//|---@field maximum_visibility integer Saved map sensor range ceiling
//|---@field terrain_ready boolean Whether saved tiles have a valid dictionary.
// lua-types-end

// lua-types-begin btech 00010
//|---@class StoredBattleUnit
//|---@field name string
//|---@field template string
//|---@field class_code integer Saved unit class, including deferred classes.
//|---@field movement_code integer Saved movement type, including deferred types.
//|---@field tons integer
//|---@field map integer|nil
// lua-types-end

// lua-types-begin btech 00011
//|---@alias BattleWeapon "arrow_iv"|"clan_arrow_iv"|"long_tom"|"sniper"|"thumper"|"long_tom_cannon"|"sniper_cannon"|"thumper_cannon"|"a_pod"|"clan_a_pod"|"i_narc_beacon"|"narc_beacon"|"clan_narc_beacon"|"anti_missile_system"|"clan_anti_missile_system"|"laser_ams"|"clan_laser_ams"|"clan_lbx2"|"clan_lbx5"|"clan_lbx10"|"clan_lbx20"|"clan_ultra_ac2"|"clan_ultra_ac5"|"clan_ultra_ac10"|"clan_ultra_ac20"|"mml3"|"mml5"|"mml7"|"mml9"|"clan_atm3"|"clan_atm6"|"clan_atm9"|"clan_atm12"|"clan_lrm5"|"clan_lrm10"|"clan_lrm15"|"clan_lrm20"|"clan_srm2"|"clan_srm4"|"clan_srm6"|"clan_streak_srm2"|"clan_streak_srm4"|"clan_streak_srm6"|"clan_streak_lrm5"|"clan_streak_lrm10"|"clan_streak_lrm15"|"clan_streak_lrm20"|"clan_gauss_rifle"|"clan_machine_gun"|"clan_light_machine_gun"|"clan_heavy_machine_gun"|"clan_er_large_laser"|"clan_er_medium_laser"|"clan_er_small_laser"|"clan_er_micro_laser"|"clan_er_ppc"|"clan_flamer"|"clan_heavy_large_laser"|"clan_heavy_medium_laser"|"clan_heavy_small_laser"|"clan_large_pulse_laser"|"clan_medium_pulse_laser"|"clan_small_pulse_laser"|"clan_micro_pulse_laser"|"clan_er_large_pulse_laser"|"clan_er_medium_pulse_laser"|"clan_er_small_pulse_laser"|"clan_plasma_rifle"|"flamer"|"coolant_gun"|"heavy_flamer"|"vehicle_flamer"|"vehicle_heavy_flamer"|"plasma_rifle"|"acid_thrower"|"thunderbolt5"|"thunderbolt10"|"thunderbolt15"|"thunderbolt20"|"hyper_ac2"|"hyper_ac5"|"hyper_ac10"|"machine_gun"|"heavy_machine_gun"|"light_ac2"|"light_ac5"|"small_laser"|"medium_laser"|"large_laser"|"ppc"|"er_small_laser"|"er_medium_laser"|"er_large_laser"|"er_ppc"|"small_pulse_laser"|"medium_pulse_laser"|"large_pulse_laser"|"x_small_pulse_laser"|"x_medium_pulse_laser"|"x_large_pulse_laser"|"light_ppc"|"heavy_ppc"|"snub_nosed_ppc"|"srm2"|"rocket10"|"rocket15"|"rocket20"|"mrm10"|"mrm20"|"mrm30"|"mrm40"|"streak_srm2"|"streak_srm4"|"streak_srm6"|"lr_dfm5"|"lr_dfm10"|"lr_dfm15"|"lr_dfm20"|"sr_dfm2"|"sr_dfm4"|"sr_dfm6"|"elrm5"|"elrm10"|"elrm15"|"elrm20"|"lrm5"|"lrm10"|"lrm15"|"srm4"|"srm6"|"lrm20"|"heavy_gauss_rifle"|"gauss_rifle"|"light_gauss_rifle"|"magshot_gauss_rifle"|"lbx2"|"lbx5"|"lbx10"|"lbx20"|"ac2"|"ac5"|"ac10"|"ac20"|"ultra_ac2"|"ultra_ac5"|"ultra_ac10"|"ultra_ac20"|"rotary_ac2"|"rotary_ac5"|"clan_rotary_ac2"|"clan_rotary_ac5"|"clan_rotary_ac10"|"clan_rotary_ac20"
// lua-types-end

// lua-types-begin btech 00012
//|---@class BattleCriticalLocation
//|---@field section BattleSectionName
//|---@field slot integer Zero-based critical slot.
// lua-types-end

// lua-types-begin btech 00013
//|---@alias BattleAmmunitionMode "smoke"|"mine"|"i_narc_explosive"|"i_narc_haywire"|"i_narc_ecm"|"i_narc_nemesis"|"semi_guided"|"swarm"|"swarm1"|"stinger"|"narc"|"normal"|"cluster"|"artemis"|"precision"|"flechette"|"armor_piercing"|"caseless"|"incendiary"|"inferno"|"mml_lrm"|"mml_lrm_artemis"|"mml_lrm_narc"|"mml_lrm_swarm"|"mml_lrm_swarm1"|"mml_lrm_semi_guided"|"mml_lrm_stinger"|"extended_range"|"high_explosive"
// lua-types-end

// lua-types-begin btech 00014
//|---@alias BattleFireMode "normal"|"heat"|"hotload"|"ultra"|"rapid"|"rotary2"|"rotary4"|"rotary6"|"gatling"
// lua-types-end

// lua-types-begin btech 00015
//|---@class BattleWeaponMount
//|---@field weapon BattleWeapon
//|---@field criticals BattleCriticalLocation[] Slots belonging to one weapon.
//|---@field one_shot boolean Self-contained launcher; does not draw from ammunition bins.
//|---@field initially_spent boolean Initial template supply already expended.
//|---@field initial_ammunition_mode BattleAmmunitionMode
//|---@field initial_fire_mode BattleFireMode Template mode; live mode is reported by unit.weapons.
//|---@field rear_mount boolean
//|---@field on_targeting_computer boolean Explicit authored link, separate from automatic eligibility.
//|---@field brand integer|nil Manufacturer metadata.
// lua-types-end

// lua-types-begin btech 00016
//|---@class BattleAmmunitionBin
//|---@field location BattleCriticalLocation
//|---@field weapon BattleWeapon
//|---@field rounds integer Initial salvos in this independent bin.
//|---@field capacity integer Installed bin capacity in salvos.
//|---@field hotload boolean Retained bin flag; does not hotload the launcher.
//|---@field half_ton boolean Explicit half-ton construction flag.
//|---@field mode BattleAmmunitionMode
//|---@field brand integer|nil
// lua-types-end

// lua-types-begin btech 00017
//|---@class BattleSystemCritical
//|---@field location BattleCriticalLocation
//|---@field system string Snake_case system identity.
//|---@field brand integer|nil
// lua-types-end

// lua-types-begin btech 00018
//|---@class BattleLoadout
//|---@field weapons BattleWeaponMount[]
//|---@field ammunition BattleAmmunitionBin[]
//|---@field systems BattleSystemCritical[]
// lua-types-end

// lua-types-begin btech 00019
//|local btech_template = {}
// lua-types-end

// lua-types-begin btech 00022
//|---@class BattleCargoTransferPoint
//|---@field x integer Zero-based map column.
//|---@field y integer Zero-based map row.
//|---@field reveal_hint boolean|nil Defaults to false; disclose coordinates in location failures only when true.
// lua-types-end

// lua-types-begin btech 00023
//|local btech_map = {}
// lua-types-end

// lua-types-begin btech 00031
//|---@class BattleRadioChannel
//|---@field frequency integer Frequency from 0 through 999999.
//|---@field title string At most fifteen UTF-8 bytes.
//|---@field mode {digital: boolean, muted: boolean, relay: boolean, info: boolean, scan: boolean, color: string?}
// lua-types-end

// lua-types-begin btech 00032
//|local btech_unit = {}
// lua-types-end

// lua-types-begin btech 00033
//|---@class BattleVtolFuelStatus
//|---@field original_capacity integer Template fuel capacity.
//|---@field capacity integer Current capacity including 2000 per installed or carried auxiliary tank.
//|---@field remaining integer Saved fuel; -1 indicates announced exhaustion.
//|---@field auxiliary_tanks integer Carried Fuel_Tank items across manufacturers.
//|---@field installed_tanks integer Fuel_Tank criticals in saved VTOL construction.
//|---@field excess_mass integer Fuel above original capacity in 1/1024 tons before cargo discounts.
// lua-types-end

// lua-types-begin btech 00037
//|---@class BattleSectionState
//|---@field armor integer
//|---@field internal integer
//|---@field rear integer
// lua-types-end

// lua-types-begin btech 00038
//|---@class BattlePosition
//|---@field map integer Battlefield object dbref.
//|---@field x integer Zero-based column.
//|---@field y integer Zero-based row.
// lua-types-end

// lua-types-begin btech 00039
//|---@class BattlePower
//|---@field state "off"|"starting"|"running"
//|---@field remaining integer|nil Remaining committed seconds during startup.
// lua-types-end

// lua-types-begin btech 00040
//|---@class BattlePoint
//|---@field x number
//|---@field y number
// lua-types-end

// lua-types-begin btech 00041
//|---@class BattleMotion
//|---@field point table Continuous x/y measured in hex heights.
//|---@field heading number Current clockwise compass heading.
//|---@field desired_heading number
//|---@field speed number Current kph, negative for reverse.
//|---@field desired_speed number
// lua-types-end

// lua-types-begin btech 00042
//|---@class BattleMobility
//|---@field maximum_speed number Damage-adjusted maximum kph before terrain, heat and cargo.
//|---@field piloting_modifier integer Damage modifier for subsequent piloting checks.
// lua-types-end

// lua-types-begin btech 00043
//|---@alias BattleDetectionChannel "sensors"|"sight"|"radar"|"probe" Values from btech.unit.detection_channels.
//|---@alias BattlePerceptionStatus "ready"|"degraded"|"jammed"|"damaged"|"disabled"|"absent"
// lua-types-end

// lua-types-begin btech 00044
//|---@class BattleTargetLock
//|---@field target integer Selected unit dbref; may no longer be visible.
//|---@field remaining integer Settling seconds, 0..8; zero does not establish visibility.
// lua-types-end

// lua-types-begin btech 00045
//|---@class BattleStaggerHit
//|---@field damage integer
//|---@field remaining integer Seconds until this incoming damage group expires.
//|---@field counted boolean Whether a rolling check already used this group.
// lua-types-end

// lua-types-begin btech 00046
//|---@class BattleStagger
//|---@field action_damage integer Restored signed action-time scalar, independent of incoming damage history.
//|---@field hits BattleStaggerHit[]
//|---@field elapsed integer Committed seconds since the last rolling check.
//|---@field turn_damage integer Unchecked traditional damage.
//|---@field phase integer Saved per-unit turn phase, 0 through 29.
//|---@field checked_phase integer? Phase of the previous traditional check.
// lua-types-end

// lua-types-begin btech 00047
//|---@class BattleStandTimer
//|---@field state "rising"|"recovering"
//|---@field remaining integer Committed seconds remaining, 1..60.
// lua-types-end

// lua-types-begin btech 00048
//|---@class BattleMass
//|---@field engine integer Engine mass in 1/1024 tons.
//|---@field cockpit integer
//|---@field gyro integer Signed gyro accounting for destroyed center torsos.
//|---@field structure integer
//|---@field armor integer
//|---@field equipment integer
//|---@field ammunition integer
//|---@field cargo integer Authored cargo-space installation mass, excluding loose stock.
//|---@field total integer Current total in 1/1024 tons.
// lua-types-end

// lua-types-begin btech 00049
//|---@alias BattleLateralMode "none" | "front_left" | "front_right" | "rear_left" | "rear_right"
// lua-types-end

// lua-types-begin btech 00050
//|---@class BattleLateralState
//|---@field active BattleLateralMode
//|---@field pending BattleLateralMode?
//|---@field remaining integer Seconds until pending direction activates.
// lua-types-end

// lua-types-begin btech 00051
//|---@class BattleTransportState
//|---@field altitude number|nil Continuous altitude in terrain levels, including carried and pending-descent fractions.
//|---@field fortified boolean Scenario emplacement; blocks movement and towing, counts as immobile for aiming.
//|---@field towable boolean Explicit permission to tow this unit out of character.
//|---@field towing integer|nil Unit currently carried by this unit.
//|---@field towed_by integer|nil Carrier currently towing this unit.
//|---@field orbital_drop {elevation: integer, protection: {state: "cocoon"|"jump_jets", integrity: integer?}}|nil Saved orbital descent; inspection does not advance it.
//|---@field free_fall {elevation: number, speed: integer, remaining: integer, grounded: boolean}|nil Pending descent; inspection does not advance it.
// lua-types-end

// lua-types-begin btech 00052
//|---@class BattleHullDownState
//|---@field active boolean Completed lowered posture.
//|---@field pending boolean|nil Lowering (true) or raising (false).
//|---@field remaining integer Seconds left in the transition; zero when idle.
// lua-types-end

// lua-types-begin btech 00053
//|---@class BattleRadioState
//|---@field radio_experience_remaining integer Saved communication XP gate, 0 through 61 seconds.
//|---@field radio_skill integer Communication target captured on startup.
//|---@field radio BattleRadioChannel[] Active channels, indexed from one in this inspection array.
//|---@field radio_capabilities {channels: integer, range: integer, relay: boolean, digital: boolean, info: boolean, scan: boolean} Derived installed hardware limits.
// lua-types-end

// lua-types-begin btech 00054
//|---@class BattleUnitState: BattleTransportState, BattleRadioState
//|---@field mw_safety boolean MechWarrior safety; enabled when startup completes.
//|---@field bth_debug boolean Retained debug preference; combat reports do not consume this flag.
//|---@field last_startup integer Unix time of the last completed startup; zero before first completion.
//|---@field cockpit_links integer[] Three explicit cockpit destinations; unresolved references remain saved.
//|---@field preferred_id string? Configured two-letter preference; separate from the currently assigned ID.
//|---@field hull_down BattleHullDownState Quad hull-down posture and transition.
//|---@field kind "mech"
//|---@field mass BattleMass Derived current mass; detached from world state.
//|---@field searchlight_warning boolean Notify occupants on external illumination transitions.
//|---@field lateral BattleLateralState
//|---@field autocon_shutdown boolean Include shutdown targets in routine notices; defaults false.
//|---@field armor_warning boolean Armor threshold warnings; enabled by default.
//|---@field ammunition_warning boolean Low-ammunition warnings; enabled by default.
//|---@field friendly_fire_safety boolean Reject non-coolant fire at teammates; off by default.
//|---@field null_signature BattleSignatureState
//|---@field stealth BattleSignatureState
//|---@field electronics BattleElectronics Selected suite modes and last committed field.
//|---@field beacons table<BattleSection, BattleBeaconKind[]> Attached effects grouped by section.
//|---@field narc_sections BattleSection[] Sections carrying homing beacons.
//|---@field ams_enabled boolean Automatic anti-missile defense switch.
//|---@field auto_fall boolean Skip downhill cliff avoidance when piloted.
//|---@field hex_sync_pending boolean A collision interrupted synchronization of motion.point and position.
//|---@field elevation integer|nil Current signed altitude with terrain-effect jump rounding; nil when unplaced.
//|---@field stand_timer BattleStandTimer?
//|---@field reactor_instability_remaining integer? Damage window ticks remaining; nil uses initial world startup grace.
//|---@field triple_myomer_active boolean Derived from installed myomer and sampled excess heat.
//|---@field movement_maximum_speed number Current throttle ceiling, including active myomer.
//|---@field charge {target: integer?, elapsed: integer, distance: number} Persistent charge intent and movement counters.
//|---@field limb_recycle table<string, integer> Remaining physical recovery seconds by limb.
//|---@field stagger BattleStagger
//|---@field posture "standing"|"prone"
//|---@field flooded_sections string[] Persistent flooded section names.
//|---@field breached_sections string[] Persistent vacuum-disabled section names.
//|---@field map_slot integer|nil Persisted battlefield membership order.
//|---@field aimed_section BattleAimSelection|nil Saved anatomy preference; independent of the current lock.
//|---@field target_lock BattleTargetLock|BattleHexLock|nil
//|---@field sensor_ranges {tactical: integer, long_range: integer, scan: integer} Computer-derived hex limits after sensor damage.
//|---@field observer boolean Administrator-assigned observer role.
//|---@field combat_safe boolean Operator-imposed immunity to combat damage.
//|---@field weapons_hold boolean Operator-imposed firing restriction; mechanical readiness is independent.
//|---@field visibility {invisible: boolean, clairvoyant: boolean} Operator visibility state.
//|---@field battlefield_id string? Current battlefield identity; absent without map membership.
//|---@field searchlight {on: boolean, destroyed: boolean, remaining: integer, mode: "auto"|"on"|"off"} Hardware, pending five-second switch and switching policy.
//|---@field fired_recently boolean Launched a weapon since the last heartbeat.
//|---@field spotter integer? Self ID while spotting, otherwise the selected observer.
//|---@field artillery_adjustment integer Saved correction for the current artillery target.
//|---@field spotter_events BattleSpotterEvents Pending radio requests and periodic checks.
//|---@field tag BattleTagState
//|---@field signature {team: integer, hidden: boolean, illuminated: boolean} Team, hiding and scenario lighting.
//|---@field scanner_perception integer Perception captured at startup completion.
//|---@field facing {torso: "left"|"center"|"right"|"both", arms_flipped: boolean}
//|---@field stun_remaining integer Remaining seconds of cockpit stun.
//|---@field pilot_injuries integer Tactical injury count; six means scenario pilot loss.
//|---@field self_destruct {remaining: integer, ammunition: boolean}|nil Admitted timer and its actual Mech detonation mode.
//|---@field self_destruct_safe boolean Scenario protection from new ammunition self-destruct requests.
//|---@field hide_elapsed integer|nil Elapsed camouflage checks; nil when no hide event is pending.
//|---@field crew_recovery_remaining integer Empty-crew consciousness countdown; random state stays private.
//|---@field character_pilot {injuries: integer, killed: boolean}? Saved character-mode injury status; character health determines death.
//|---@field heat_cutoff {enabled: boolean, disabled: integer, remaining: integer|nil} Intentional cooling suppression and seconds until the toggle completes.
//|---@field last_jump {heading: integer, length: integer} Current course bearing and signed length in field units, retained after landing.
//|---@field heat_sample {production: number, dissipation: number} Last committed thermal sample; production includes stored weapon heat.
//|---@field heat {stored: number, excess: number} Weapon heat (possibly negative coolant credit until the next sample) and sampled excess heat.
//|---@field inferno_remaining integer Saved burn seconds; cooling is reduced by six while positive.
//|---@field overheat_clock {elapsed: integer, phase: integer, injury_due: boolean} Saved committed-second thermal checks.
//|---@field weapon_recycle table<integer, integer> Remaining seconds keyed by zero-based weapon index.
//|---@field component_failures {location: table, failure: string}[] Nonweapon diagnostic conditions; material damage determines system operation.
//|---@field weapon_failures table<integer, "jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam"> Temporary conditions by mount index; existing recycle clocks govern recovery.
//|---@field gyro "standard"|"hardened"|"xl"|"compact" Construction family.
//|---@field artemis BattleArtemisController[] Installed controllers and resolved links.
//|---@field weapon_damage BattleWeaponDamage[]|nil Mech weapon critical degradation.
//|---@field masc BattleBoosterState Saved activation and overload/recovery state.
//|---@field supercharger BattleBoosterState Independent compressor timer and failure state.
//|---@field supercharger_installed boolean Template technology flag.
//|---@field supercharger_operational boolean Technology remains available and has not failed.
//|---@field c3_members integer[] Classic C3 members retained by current working-master capacity.
//|---@field c3_operational boolean Working classic C3 hardware.
//|---@field c3i_members integer[] Eligible network members including this unit, empty when disconnected; shutdown and ECM retain membership.
//|---@field c3_hardware {masters: integer, working_masters: integer, slave_installed: boolean, slave_operational: boolean, c3i_installed: boolean, c3i_operational: boolean} Installed and working command-network computers; independent of power and membership.
//|---@field masc_installed boolean Sufficient MASC hardware is installed.
//|---@field masc_operational boolean Enough MASC slots remain functional; does not indicate activation.
//|---@field unjam BattleUnjam|nil Active feed recovery.
//|---@field dumping table|nil Active ammunition selection and elapsed cadence.
//|---@field gyro_damage integer Effective gyro damage after hardened protection.
//|---@field mobility BattleMobility
//|---@field jump_capacity {speed: number, movement_points: integer} Damage/gravity-adjusted capacity; does not authorize flight. Unplaced units use 100% gravity.
//|---@field flight {path: {start: BattlePoint, end: BattlePoint, start_elevation: number, end_elevation: integer, movement_points: integer, continuation: boolean, projection: {bearing: integer, range: number}|nil, target_range: number|nil}, travelled: number, completed_distance: number, landing_requested: boolean, sampled_movement_points: integer, dfa_target: integer|nil}|nil
//|---@field airborne {point: BattlePoint, elevation: number}|nil Last committed airborne sample.
//|---@field jump_stabilization integer Remaining seconds, zero through twelve.
//|---@field engine "standard"|"light"|"xl"|"xxl"|"compact" Installed fusion-engine family.
//|---@field destroyed boolean Core structure, cockpit or engine is destroyed.
//|---@field lost_criticals BattleCriticalLocation[] Explicit destroyed equipment slots.
//|---@field motion BattleMotion|nil
//|---@field power BattlePower
//|---@field pilot integer|nil Player in the cockpit; must be physically inside this unit.
//|---@field position BattlePosition|nil
//|---@field definition BattleTemplate Owned definition, independent of source files.
//|---@field sections table<BattleSectionName, BattleSectionState>
//|---@field ammunition integer[] Remaining salvos in resolved bin order.
// lua-types-end

// lua-types-begin btech 00055
//|---@class BattleVehicleMass
//|---@field engine integer
//|---@field cockpit integer
//|---@field components integer
//|---@field turret integer
//|---@field structure integer
//|---@field armor integer
//|---@field equipment integer
//|---@field cooling integer
//|---@field cargo integer
//|---@field ammunition integer Loaded ammunition mass.
//|---@field ammunition_capacity integer Full surviving bin mass.
//|---@field total integer Current physical mass in 1/1024-ton units.
//|---@field design_total integer Current component total with full surviving bins.
// lua-types-end

// lua-types-begin btech 00056
//|---@class BattleDigState
//|---@field dug_in boolean Whether cover applies.
//|---@field digging boolean Whether preparation is active.
//|---@field completion integer[] Pending completion deadlines in seconds; empty means none.
// lua-types-end

// lua-types-begin btech 00057
//|---@class BattleVehicleState: BattleTransportState, BattleRadioState
//|---@field armor_warning boolean Armor severity warnings; enabled by default.
//|---@field ammunition_warning boolean Low-ammunition warnings; enabled by default.
//|---@field searchlight {on: boolean, destroyed: boolean, remaining: integer, mode: "auto"|"on"|"off"} Hardware, pending five-second switch and switching policy.
//|---@field autocon_shutdown boolean Include shutdown targets in routine contact notices.
//|---@field searchlight_warning boolean Announce external illumination transitions.
//|---@field mw_safety boolean MechWarrior safety; enabled when startup completes.
//|---@field bth_debug boolean Retained debug preference; combat reports do not consume this flag.
//|---@field last_startup integer Unix time of the last completed startup; zero before first completion.
//|---@field cockpit_links integer[] Three explicit cockpit destinations; unresolved references remain saved.
//|---@field preferred_id string? Configured two-letter preference; separate from the currently assigned ID.
//|---@field fuel BattleVtolFuelStatus|nil Live fuel projection for VTOLs only.
//|---@field fired_recently boolean A weapon launched since the last heartbeat.
//|---@field observer boolean Administrator-assigned observer role.
//|---@field combat_safe boolean Operator-imposed immunity to combat damage.
//|---@field weapons_hold boolean Operator-imposed firing restriction; mechanical readiness is independent.
//|---@field visibility {invisible: boolean, clairvoyant: boolean} Operator visibility state.
//|---@field dig BattleDigState Saved ground-vehicle cover preparation.
//|---@field mass BattleVehicleMass Derived from current material and ammunition; units are 1/1024 ton.
//|---@field inferno_remaining integer Stationary-unit jelly duration.
//|---@field burning_sections table<string, integer> Section fire countdowns in seconds.
//|---@field extinguishing integer|nil Seconds until the crew completes its attempt.
//|---@field pod_removal integer|nil Remaining seconds of the crew iNarc-removal attempt.
//|---@field beacons table<string, string[]> Attached effects keyed by surviving vehicle section.
//|---@field ams_enabled boolean Saved automatic anti-missile defense switch.
//|---@field artemis BattleArtemisController[] Installed controllers and resolved links.
//|---@field unjam BattleUnjam|nil Active feed clearing attempt.
//|---@field weapon_recycle table<integer, integer> Countdown seconds by zero-based weapon index.
//|---@field spent_launchers integer[] Expended zero-based one-shot weapon indices.
//|---@field lost_criticals table[] Destroyed vehicle equipment locations, each with section and zero-based slot.
//|---@field piloting_damage integer Cumulative vehicle handling penalty.
//|---@field component_failures {location: table, failure: string}[] Nonweapon diagnostic conditions; material damage determines system operation.
//|---@field weapon_failures table<integer, "jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam"> Temporary conditions by mount index; existing recycle clocks govern recovery.
//|---@field crew_stun_remaining integer Seconds until the pending recovery event; zero means none.
//|---@field crew_stunned boolean Effective crew stun, independent of its timer.
//|---@field self_destruct {remaining: integer, ammunition: boolean}|nil Admitted timer and its actual Mech detonation mode.
//|---@field self_destruct_safe boolean Scenario protection from new ammunition self-destruct requests.
//|---@field hide_elapsed integer|nil Elapsed camouflage checks; nil when no hide event is pending.
//|---@field crew_recovery_remaining integer Empty-crew consciousness countdown, separate from crew stun.
//|---@field weapon_heat number Passive weapon heat and coolant credit; ground vehicles do not overheat.
//|---@field gunnery_damage integer Cumulative firing penalty from sensor and commander damage.
//|---@field lost_stabilizers string[] Sections with destroyed weapon stabilizers.
//|---@field signature {team: integer, hidden: boolean, illuminated: boolean} Team, hiding and scenario lighting.
//|---@field scanner_perception integer Perception captured at startup completion.
//|---@field sensor_ranges {tactical: integer, long_range: integer, scan: integer} Computer-derived hex limits.
//|---@field aimed_section BattleAimSelection|nil Saved anatomy preference; independent of the current lock.
//|---@field target_lock BattleTargetLock|BattleHexLock|nil Saved selection and settling countdown.
//|---@field artillery_adjustment integer Saved correction for the selected artillery coordinate.
//|---@field c3_hardware {masters: integer, working_masters: integer, slave_installed: boolean, slave_operational: boolean, c3i_installed: boolean, c3i_operational: boolean} Installed and working command-network computers; independent of power and membership.
//|---@field c3i_members integer[] Eligible members in the improved command network.
//|---@field c3_members integer[] Eligible members in the classic command network.
//|---@field flooded boolean Permanently disabled by water, independently of armor and crew health.
//|---@field breached_sections string[] Persisted vacuum breaches; equipment is disabled without destroying slots or expending ammunition.
//|---@field crew_killed boolean Instant crew loss, independent of tactical and character injury counts.
//|---@field electronics BattleElectronics Selected suite modes and last committed field.
//|---@field spotter integer|nil Self declares spotting; another unit selects a forward observer.
//|---@field spotter_events BattleSpotterEvents Pending radio requests and periodic checks.
//|---@field tag BattleTagState Shared TAG selection and lock/recycle countdown.
//|---@field character_pilot {injuries: integer, killed: boolean}? Saved RPG pilot status, independent of tactical injury count.
//|---@field friendly_fire_safety boolean Pilot-selected teammate protection.
//|---@field auto_fall boolean Skip downhill cliff avoidance when piloted.
//|---@field brief BattleBriefSettings
//|---@field fire_modes table<integer, BattleFireMode> Selected non-normal firing modes by zero-based weapon index.
//|---@field ammunition_modes table<integer, BattleAmmunitionMode> Selected non-normal modes by zero-based weapon index.
//|---@field turret_heading number|nil Absolute heading of a surviving turret.
//|---@field automatic_turret boolean Pilot-selected automatic unit/hex target tracking.
//|---@field turret_jammed boolean Recoverable turret rotation damage.
//|---@field turret_repairs integer[] Pending 60-second repair attempts.
//|---@field turret_locked boolean Turret damage prevents rotation.
//|---@field maximum_speed number Current maximum kph after motive damage.
//|---@field motive_speed_loss number Maximum speed lost to motive damage in kph.
//|---@field immobilized boolean Motive-system destruction prevents ground motion.
//|---@field under_bridge boolean Hovercraft beneath a bridge span.
//|---@field elevation integer|nil Ground support height; hovercraft float at water level.
//|---@field motion BattleMotion|nil
//|---@field pilot integer|nil Assigned cockpit operator.
//|---@field power BattlePower
//|---@field kind "vehicle"
//|---@field simulation_supported false Full vehicle terrain and combat support is unfinished.
//|---@field definition table Owned ground-vehicle definition.
//|---@field sections table<string, BattleSectionState> Vehicle faces: left, right, front, rear, turret.
//|---@field ammunition integer[] Remaining rounds in resolved bin order.
//|---@field position BattlePosition|nil
//|---@field map_slot integer|nil
//|---@field destroyed boolean Any hull face has lost its internal structure.
// lua-types-end

// lua-types-begin btech 00083
//|---@class BattleRange
//|---@field horizontal number Horizontal Euclidean range in hex heights.
//|---@field spatial number Euclidean range including signed ground elevation/depth.
//|---@field bearing number|nil Degrees clockwise from north; nil for coincident centers.
//|---@field hex_distance integer Minimum adjacent hex steps, without terrain costs.
// lua-types-end

// lua-types-begin btech 00088
//|---@class BattleCharacter
//|---@field perception_target integer Target derived from intuition, learning and effective Perception skill.
//|---@field values table<string, {value: integer, experience: integer, last_used: integer}> Detached named skill/advantage records.
//|---@field unconscious_remaining integer Seconds before the next recovery attempt; zero when conscious.
//|---@field bruise integer
//|---@field lethal integer
//|---@field build integer
//|---@field reflexes integer
//|---@field intuition integer
//|---@field learn integer
//|---@field charisma integer
//|local btech_character = {}
// lua-types-end

// lua-types-begin btech 00089
//|---@class BattleAdvantageDefinition
//|---@field name string Canonical advantage name.
//|---@field kind "boolean"|"ranked"|"attribute_mask" Boolean values activate only at one.
// lua-types-end

// lua-types-begin btech 00091
//|---@class BattleSkillDefinition
//|---@field name string Canonical storage name.
//|---@field category "athletic"|"mental"|"physical"|"social"
//|---@field threshold integer Default experience threshold.
//|---@field continuous boolean Whether awards bypass the thirty-second interval.
// lua-types-end

// lua-types-begin btech 00094
//|---@class BattleSkillProgress
//|---@field name string Canonical skill name.
//|---@field target integer Current skill target including stored earned levels.
//|---@field raw_target integer Skill target excluding earned levels.
//|---@field earned_levels integer Stored XP bonus.
//|---@field balance integer Low 24-bit experience balance.
//|---@field threshold integer Current runtime threshold.
//|---@field next_level_balance integer? Total balance needed for the next stored level; nil when disabled.
//|---@field remaining integer? Additional points needed; zero if recalculation is overdue.
// lua-types-end

// lua-types-begin btech 00101
//|---@class BattleHexCoordinate
//|---@field x integer
//|---@field y integer
// lua-types-end

// lua-types-begin btech 00102
//|---@class BattleSurfaceBreak
//|---@field map integer
//|---@field coordinate BattleHexCoordinate
//|---@field before BattleHex
//|---@field after BattleHex
//|---@field fall_levels integer
//|---@field falls table[] Ordered pairs of unit dbref and Mech fall report.
//|---@field vehicle_falls table[] Ordered pairs of unit dbref and vehicle fall report.
//|---@field flooded_vehicles integer[]
//|---@field notices table[] Unit dbrefs and cockpit message text.
// lua-types-end

// lua-types-begin btech 00104
//|---@class BattleMapEmitOptions
//|---@field audience? "all"|"range"|"line_of_sight" Recipient selection; defaults to all.
//|---@field origin? BattleHexCoordinate Required anchor for range and line_of_sight audiences.
//|---@field range? number Nonnegative hex radius; required with the range audience.
// lua-types-end

// lua-types-begin btech 00116
//|---@class BattleAuthoredMapLink
//|---@field parent integer Parent map.
//|---@field coordinate BattleHexCoordinate Placement on the parent.
//|---@field entrances? table[] Four cardinal modes, north/east/south/west: {kind="none"}, {kind="offset",distance=N}, or {kind="exact",coordinate={x=X,y=Y}}.
// lua-types-end

// lua-types-begin btech 00119
//|---@alias BattleMapEntrance {mode: "offset", offset: integer}|{mode: "exact", x: integer, y: integer}
// lua-types-end

// lua-types-begin btech 00120
//|---@class BattleMapEntrances
//|---@field north? BattleMapEntrance
//|---@field east? BattleMapEntrance
//|---@field south? BattleMapEntrance
//|---@field west? BattleMapEntrance
// lua-types-end

// lua-types-begin btech 00121
//|---@class BattleMapLink
//|---@field parent Object Parent map object.
//|---@field x integer Placement column on the parent.
//|---@field y integer Placement row on the parent.
//|---@field entrances? BattleMapEntrances
// lua-types-end

// lua-types-begin btech 00129
//|---@class BattleMapHexChange
//|---@field map integer
//|---@field coordinate BattleHexCoordinate
//|---@field before BattleHex
//|---@field after BattleHex
// lua-types-end

// lua-types-begin btech 00131
//|---@class BattleMapIceReport
//|---@field map integer
//|---@field changed BattleHexCoordinate[] Coordinates in column-major processing order.
//|---@field fractures BattleSurfaceBreak[] Melting consequences, including affected occupants.
// lua-types-end

// lua-types-begin btech 00134
//|---@class BattleMapEnvironment
//|---@field gravity integer Percent of Earth gravity, 0 through 255.
//|---@field temperature integer Celsius, -128 through 127.
//|---@field vacuum boolean? Defaults to false, clearing existing vacuum.
//|---@field underground boolean? Defaults to false; existing underground status is retained.
// lua-types-end

// lua-types-begin btech 00137
//|---@class BattleBlastZone
//|---@field x integer
//|---@field y integer
//|---@field radius integer
// lua-types-end

// lua-types-begin btech 00142
//|---@alias BattleTerrainName "grassland"|"road"|"light_forest"|"heavy_forest"|"water"|"ice"|"bridge"|"high_water"|"rough"|"mountains"|"fire"|"smoke"|"snow"|"building"|"wall"
// lua-types-end

// lua-types-begin btech 00145
//|---@alias BattleLineOfSight "none"|"blocked"|"clear"
// lua-types-end

// lua-types-begin btech 00147
//|---@alias BattlePlacement {x: integer, y: integer, z?: integer}
// lua-types-end

// lua-types-begin btech 00151
//|---@class BattleMapUnitFilter
//|---@field origin BattleHexCoordinate Filter anchor.
//|---@field range number Nonnegative hex radius.
// lua-types-end

// lua-types-begin btech 00155
//|---@alias BattleProbeKind "beagle"|"light"|"bloodhound"|"watchdog"
// lua-types-end

// lua-types-begin btech 00156
//|---@alias BattleContactArc "front" | "right" | "rear" | "left"
// lua-types-end

// lua-types-begin btech 00157
//|---@class BattleContactView
//|---@field label string Battlefield label, lowercase for identified allies.
//|---@field coordinate BattleHexCoordinate
//|---@field elevation integer Current elevation.
//|---@field short_text string Plain compact biped contact row.
//|---@field verbose_text string Plain multiline C0 contact report.
//|---@field identified boolean Current terrain permits identification.
//|---@field weapon_arc BattleContactArc Observer torso direction; individual weapons may have different arcs.
//|---@field detection BattleDetectionChannel|nil How the observer currently perceives this contact; nil for clairvoyant-only views.
//|---@field status string Five visible condition columns; blank behind blocking terrain.
//|---@field target integer Acquired unit dbref.
//|---@field name string Chassis name, or "something" for unidentified signals.
//|---@field friendly boolean Identified and on the same team as observer.
//|---@field range BattleRange
//|---@field network_range number|nil Closest usable command-network sighting distance; nil without an active network.
//|---@field heading number Travel axis including lateral offset; reverse speed travels opposite this axis.
//|---@field speed number Current kph.
// lua-types-end

// lua-types-begin btech 00160
//|---@class BattleSpotterEvents
//|---@field events BattleSpotterEvent[] Independent requests in insertion order; detached inspection only.
// lua-types-end

// lua-types-begin btech 00161
//|---@class BattleSpotterEvent
//|---@field order integer Global order among active events.
//|---@field remaining integer Seconds until connection completion or maintenance.
//|---@field observer integer Observer unit dbref.
//|---@field positions BattlePoint[]? Captured shooter and observer coordinates during setup; nil for maintenance.
// lua-types-end

// lua-types-begin btech 00162
//|---@class BattleTagState
//|---@field target integer? Selected target; nil during recycle.
//|---@field remaining integer Lock/recycle seconds, zero through thirty.
// lua-types-end

// lua-types-begin btech 00165
//|---@class BattleWeaponReadiness
//|---@field weapon string Conventional weapon kind.
//|---@field intact boolean
//|---@field ammunition integer Matching available salvos.
//|---@field recycle_remaining integer Simulation seconds.
//|---@field jammed boolean Ammunition-feed failure blocks firing and mode changes.
//|---@field spent boolean Self-contained salvo has already launched.
//|---@field posture_ready boolean Prone support and mounting restrictions.
//|---@field ready boolean Power, mechanical conditions, preparation and supply permit use; targeting and authority remain separate.
// lua-types-end

// lua-types-begin btech 00166
//|---@alias BattleVehicleSectionName "front"|"right"|"left"|"rear"|"turret"|"rotor"
// lua-types-end

// lua-types-begin btech 00167
//|---@class BattleWeaponInspection
//|---@field preferred_ammunition_section string|nil Canonical preferred ammunition section; fallback remains automatic.
//|---@field index integer Zero-based stable weapon number.
//|---@field name string Equipment display name.
//|---@field section BattleSectionName|BattleVehicleSectionName
//|---@field failure "jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam"|nil Temporary operational failure independent of physical integrity.
//|---@field rear_mount boolean
//|---@field one_shot boolean
//|---@field readiness BattleWeaponReadiness
//|---@field ammunition_mode BattleAmmunitionMode
//|---@field fire_mode BattleFireMode
// lua-types-end

// lua-types-begin btech 00180
//|---@class BattleAimModifiers
//|---@field self_target boolean Coolant self-application bypasses contact acquisition.
//|---@field indirect {spotter: integer, spotting: integer, movement: integer, target_lock: integer}|nil Observer contributions; perception then describes the spotter's view.
//|---@field gunnery integer
//|---@field distance number
//|---@field network_range {kind: "c3"|"c3i", distance: number, source: integer|nil}|nil Active command-network range; physical limits and firing visibility remain separate.
//|---@field range {bracket: string, modifier: integer}|nil
//|---@field attacker_movement integer
//|---@field attacker_water integer Plus one when firing at a unit from below the water surface.
//|---@field woods_cover integer Configured occupied-forest accuracy credit: zero, minus one or minus two.
//|---@field target_movement integer Movement contribution, including +1 for a VTOL with nonzero horizontal or vertical speed.
//|---@field dug_in integer Configured cover modifier, shared by Mech and vehicle attackers.
//|---@field orbital_drop integer Minus two while the target has an intact cocoon; zero after a breach.
//|---@field heat integer
//|---@field sensors integer
//|---@field control_damage integer Vehicle commander/sensor critical penalties.
//|---@field mounting_section integer
//|---@field targeting_computer integer Eligible computer fire: -1 normally, +3 for a selected section on a mobile target.
//|---@field aimed_section integer Head aim penalty: 7 against immobile Mechs, 25 against mobile Mechs; overrides computer assistance.
//|---@field beacon_accuracy integer Haywire interference and iNarc homing assistance.
//|---@field ammunition_accuracy integer Selected ammunition adjustment; LB-X cluster is -3 versus VTOLs, otherwise -1; Stinger is -3 versus flying VTOLs and -1 during orbital descent.
//|---@field targeting_mode integer Scenario tracking-mode adjustment, separate from installed computer equipment.
//|---@field weapon_accuracy integer Intrinsic accuracy adjustment; pulse lasers contribute -2, MRMs +1.
//|---@field weapon_damage integer Penalty from damaged focusing, ranging and other weapon components.
//|---@field target_lock integer
//|---@field perception {channel: BattleDetectionChannel|nil, direct_fire: boolean, modifier: integer}|nil Nil without a current contact; direct_fire is false behind blocking terrain.
// lua-types-end

// lua-types-begin btech 00181
//|---@class BattleSectionExposureReport
//|---@field cause "water"|"vacuum"
//|---@field section BattleSectionName
//|---@field reactor_explosion table|nil
//|---@field fall table|nil
//|---@field notices {unit: integer, text: string}[]
// lua-types-end

// lua-types-begin btech 00182
//|---@class BattleTacticalImpact
//|---@field impact table Ordered material damage, critical losses and exposures (BattleSectionExposureReport[]).
//|---@field pilot_injuries table[] Applied crew consequences.
//|---@field notices table[] Cockpit messages.
//|---@field balance table[] Applied balance checks and falls.
//|---@field flooding table[] Applied flooding consequences.
// lua-types-end

// lua-types-begin btech 00183
//|---@class BattleAmmunitionDraw
//|---@field bin_index integer Zero-based bin index.
//|---@field rounds integer
// lua-types-end

// lua-types-begin btech 00184
//|---@class BattleWeaponUse
//|---@field damage_penalty integer Energy damage lost to focusing damage.
//|---@field critical_failure "barrel"|"crystal"|"feed"|nil Component responsible for a failed launch.
//|---@field weapon string
//|---@field ammunition BattleAmmunitionDraw[] Actual live-bin expenditure.
//|---@field fire_mode BattleFireMode Effective mode after supply fallback.
//|---@field heat integer Already applied; do not add this heat again.
//|---@field gatling_damage integer|nil Supply-limited gatling damage before glancing.
//|---@field ammunition_mode BattleAmmunitionMode
// lua-types-end

// lua-types-begin btech 00185
//|---@class BattleSalvoGroup
//|---@field damage integer
//|---@field hit {section: BattleSectionName, rear_armor: boolean, through_armor_critical: boolean, crew_stun: boolean}
//|---@field impact table Ordered material phases, critical losses, exposures (BattleSectionExposureReport[]), dump_ignitions, plasma_heat rolls, searchlight_destroyed and remaining scenario effects.
//|---@field pilot_injuries table[] Applied tactical injuries and consciousness results.
//|---@field notices {unit: integer, text: string}[] Already staged by unit.fire.
//|---@field balance table[] Applied balance checks and any nested falls.
//|---@field flooding table[] Applied section flooding and any nested falls.
// lua-types-end

// lua-types-begin btech 00186
//|---@class BattleInfernoHit
//|---@field target integer
//|---@field missiles integer Surviving missiles after interception.
//|---@field burn_seconds integer Duration added before immersion.
//|---@field extinguished boolean
//|---@field notices BattleNotice[]
// lua-types-end

// lua-types-begin btech 00187
//|---@class BattleWoodlandImpact
//|---@field map integer
//|---@field coordinate BattleHexCoordinate
//|---@field effect table Ignition duration, replacement terrain, or no effect.
//|---@field notices BattleNotice[]
// lua-types-end

// lua-types-begin btech 00188
//|---@class BattleWoodsAbsorption
//|---@field damage_before integer Damage supplied to terrain before absorption: per shell for direct/burst fire, total for missiles after glancing cluster adjustment and interception.
//|---@field damage_after integer Remaining armor damage: minimum one per shell before glancing for direct/burst hits; whole-projectile totals may be zero.
//|---@field terrain BattleWoodlandImpact Committed ignition or clearing check.
//|---@field notices BattleNotice[] Ordered absorption and terrain feedback.
// lua-types-end

// lua-types-begin btech 00189
//|---@class BattleSalvoReport
//|---@field initial_woods BattleWoodsAbsorption|nil Nominal LBX terrain check before pellet counting and absorption.
//|---@field woods BattleWoodsAbsorption|nil Occupied-woods consequences for direct shells (including bursts) or missile/pellet armor damage, after missile interception.
//|---@field missiles_before_defense integer|nil Cluster hits before automatic defenses.
//|---@field cluster_roll integer|nil Original missile cluster roll; nil for direct non-missile hits.
//|---@field inferno BattleInfernoHit|nil Burning replaces armor damage.
//|---@field groups BattleSalvoGroup[]
// lua-types-end

// lua-types-begin btech 00190
//|---@class BattleCharacterValue
//|---@field value integer Trained skill level.
//|---@field experience integer Encoded earned levels and XP balance.
//|---@field last_used integer Last accepted award timestamp.
// lua-types-end

// lua-types-begin btech 00191
//|---@class BattleExperienceAward
//|---@field accepted boolean
//|---@field before BattleCharacterValue
//|---@field after BattleCharacterValue
// lua-types-end

// lua-types-begin btech 00192
//|---@class BattlePilotingCheck
//|---@field skill integer Base pilot skill target.
//|---@field damage integer Penalty from physical damage.
//|---@field cockpit integer Small cockpit construction penalty, independent of damage.
//|---@field situational integer Caller-supplied modifier.
//|---@field absent_character_pilot integer Penalty for an absent in-character pilot.
//|---@field target integer Total required roll.
//|---@field roll integer|nil No dice when already prone or unable to act.
//|---@field success boolean
//|---@field experience BattleExperienceAward|nil Accepted or rate-limited skill mutation for XP-awarding callers.
// lua-types-end

// lua-types-begin btech 00193
//|---@class BattleRecoilReport
//|---@field experience_messages BattleChannelMessage[] Accepted recoil XP diagnostics published with the shot.
//|---@field check BattlePilotingCheck
//|---@field fall BattleFallReport|nil
// lua-types-end

// lua-types-begin btech 00194
//|---@class BattleAmsReport
//|---@field weapon_index integer Zero-based defensive weapon index.
//|---@field ammunition_bin integer Selected normal-ammunition bin.
//|---@field roll integer Interception capacity before rack and cluster limits.
//|---@field ammunition_spent integer May be less than interception capacity.
//|---@field shot_down integer Actual intercepted hits after the missile meets its base target number.
// lua-types-end

// lua-types-begin btech 00195
//|---@alias BattleBeaconKind "narc"|"homing"|"haywire"|"ecm"
//|---@class BattleNarcReport
//|---@field kind BattleBeaconKind
//|---@field notices BattleNotice[] Cockpit effects from the hit-location roll.
//|---@field hit boolean Whether the beacon met the full attack target.
//|---@field intercepted boolean Whether AMS intercepted the pod.
//|---@field section BattleSection|BattleVehicleSectionName|nil Surviving attachment section.
//|---@field rear boolean Rear-facing attachment notice.
// lua-types-end

// lua-types-begin btech 00196
//|---@class BattleShotReport
//|---@field launch_notices BattleNotice[] Cocoon opening feedback before target consequences.
//|---@field coordinate {x: integer, y: integer}|nil Coordinate-directed shot; target identifies the selected occupant.
//|---@field experience_messages table[] Accepted spotting/artillery awards, including misses.
//|---@field streak_confused boolean Angel interference disables Streak homing.
//|---@field narc BattleNarcReport|nil Normal beacon outcome; explosive pods use salvo damage.
//|---@field ams BattleAmsReport|nil Automatic defense activation; absent for missile rolls below base target number.
//|---@field ammunition_warning string|nil Pre-expenditure warning staged with the shot.
//|---@field shooter integer
//|---@field target integer
//|---@field weapon_index integer Zero-based stable weapon number.
//|---@field aim BattleAimModifiers
//|---@field target_number integer|nil Ordinary aim subtotal; nil beyond physical range.
//|---@field roll integer
//|---@field glancing boolean
//|---@field recoil BattleRecoilReport|nil Moving Heavy Gauss control check and fall.
//|---@field jammed boolean Recoverable ammunition-feed failure without expenditure.
//|---@field loader_destroyed boolean Permanent mount loss from loader failure or propellant ignition.
//|---@field propellant_roll integer|nil Second caseless roll; eight or more ignites propellant.
//|---@field misload BattleTacticalImpact|nil Applied misload or propellant ignition damage.
//|---@field launched boolean False for failed Streak lock: no heat/ammo expenditure, but weapon recycles.
//|---@field expenditure BattleWeaponUse
//|---@field salvo {kind: 'mech'|'vehicle'|'swarm', report: table}|nil Target-specific damage; nil on a miss or a heat-mode hit.
//|---@field heat_transfer integer Heat already added to the target, zero unless a heat-mode shot hits.
//|---@field thermal_woods BattleWoodsAbsorption|nil Terrain effects and feedback preceding thermal transfer; heat/cooling strength remains unchanged.
//|---@field missed_terrain BattleWoodlandImpact|nil Incidental terrain check after a launched non-missile miss, independent of woods damage configuration.
//|---@field cooling number? Coolant reduction applied to stored heat, including temporary negative credit.
// lua-types-end

// lua-types-begin btech 00197
//|---@class BattleVehicleShotReport
//|---@field experience_messages BattleChannelMessage[] Accepted spotting/artillery awards, including misses.
//|---@field coordinate {x: integer, y: integer}|nil Occupied-hex shot; target identifies the selected occupant.
//|---@field shooter integer
//|---@field target integer
//|---@field weapon_index integer Zero-based stable weapon number.
//|---@field aim BattleAimModifiers
//|---@field streak_confused boolean
//|---@field launch BattleVehicleLaunch
//|---@field ams BattleAmsReport|nil
//|---@field narc BattleNarcReport|nil Beacon attachment or interception; vehicle sections use their own names.
//|---@field cooling number|nil Coolant removed from target stored heat.
//|---@field heat_transfer integer Direct flamer heat added to the target, otherwise zero.
//|---@field thermal_woods BattleWoodsAbsorption|nil Terrain effects and feedback preceding thermal transfer; heat/cooling strength remains unchanged.
//|---@field missed_terrain BattleWoodlandImpact|nil Incidental terrain check after a launched non-missile miss, independent of woods damage configuration.
//|---@field salvo {kind: 'mech'|'vehicle'|'swarm', report: table}|nil Target-specific ordered damage groups.
// lua-types-end

// lua-types-begin btech 00198
//|---@class BattleVehicleInfernoHit
//|---@field missiles integer Surviving missiles after clustering and interception.
//|---@field explosion_roll integer|nil Standard mobile-vehicle heat check.
//|---@field burn_seconds integer Jelly duration added to a stationary unit.
//|---@field damage table[] Ordered initial section fire damage.
//|---@field explosion table|nil Completed heat catastrophe.
//|---@field notices BattleNotice[] Already staged by firing.
//|---@field broadcasts BattleNotice[] Raw damage broadcasts handled by firing.
// lua-types-end

// lua-types-begin btech 00199
//|---@class BattleVehicleSalvoReport
//|---@field initial_woods BattleWoodsAbsorption|nil Nominal LBX terrain check before pellet counting and absorption.
//|---@field woods BattleWoodsAbsorption|nil Occupied-woods consequences for direct shells (including bursts) or missile/pellet armor damage, after missile interception.
//|---@field experience table[] Per-packet optional pre-impact XP awards.
//|---@field experience_messages table[] Ordered XP channel diagnostics.
//|---@field cluster_roll integer|nil
//|---@field missiles_before_defense integer|nil
//|---@field groups table[] Located conventional damage packets.
//|---@field inferno BattleVehicleInfernoHit|nil Dedicated vehicle inferno outcome.
// lua-types-end

// lua-types-begin btech 00200
//|---@class BattleVehicleLaunch
//|---@field ammunition_warning string|nil Pre-expenditure warning staged with the shot.
//|---@field launch_notices BattleNotice[] Cocoon opening feedback before target consequences.
//|---@field roll integer
//|---@field hit boolean Launch classification; missile near misses may have no target effects.
//|---@field glancing boolean Tactical missile shots use the base target-number boundary.
//|---@field loader_destroyed boolean
//|---@field jammed boolean
//|---@field propellant_roll integer|nil
//|---@field misload table|nil Shooter internal damage and critical consequences.
//|---@field expenditure table Weapon, ammunition, fire mode, spent rounds, recycle and launched status.
// lua-types-end

// lua-types-begin btech 00201
//|---@class BattleArtilleryLaunchReport
//|---@field launch_notices BattleNotice[] Cocoon opening feedback before target consequences.
//|---@field shooter integer
//|---@field map integer
//|---@field coordinate {x: integer, y: integer}
//|---@field weapon_index integer
//|---@field aim {target_number: integer, maximum_range: integer, range: string}
//|---@field roll integer
//|---@field hit boolean
//|---@field launched boolean
//|---@field jammed boolean
//|---@field loader_destroyed boolean
//|---@field propellant_roll integer|nil
//|---@field expenditure BattleWeaponUse
//|---@field misload {kind: "mech"|"vehicle", report: BattleTacticalImpact|BattleVehicleInternalDamage}|nil
//|---@field ammunition_warning string|nil
//|---@field queued_shot integer|nil Persistent map queue ordinal, present after launch.
// lua-types-end

// lua-types-begin btech 00202
//|---@class BattleHexShotReport
//|---@field launch_notices BattleNotice[] Cocoon opening feedback before target consequences.
//|---@field shooter integer
//|---@field map integer
//|---@field coordinate {x: integer, y: integer}
//|---@field weapon_index integer
//|---@field aim BattleHexAimModifiers
//|---@field target_number integer|nil
//|---@field roll integer
//|---@field hit boolean
//|---@field launched boolean
//|---@field jammed boolean
//|---@field loader_destroyed boolean
//|---@field propellant_roll integer|nil
//|---@field expenditure BattleWeaponUse
//|---@field misload {kind: "mech"|"vehicle", report: BattleTacticalImpact|BattleVehicleInternalDamage}|nil
//|---@field ammunition_warning string|nil
//|---@field cluster_roll integer|nil
//|---@field terrain table[] Applied woodland effects and captured notices.
//|---@field surfaces table[] Structural rolls, optional fracture/falls, and notices.
//|---@field buildings table[] Building identity, actual damage, remaining integrity and notices.
//|---@field recoil BattleRecoilReport|nil
// lua-types-end

// lua-types-begin btech 00204
//|---@alias BattleAimSelection {class: "mech", section: string}|{class: "ground_vehicle"|"vtol", section: string}
// lua-types-end

// lua-types-begin btech 00207
//|---@class BattleSightReport
//|---@field shooter integer
//|---@field weapon_index integer
//|---@field weapon string
//|---@field target integer|nil
//|---@field coordinate BattleHexCoordinate|nil
//|---@field aim BattleAimModifiers|BattleHexAimModifiers|BattleArtilleryAim
//|---@field target_number integer|nil Nil when out of range.
//|---@field roll integer Attack dice consumed without launching.
//|---@field gatling_roll integer|nil Preparation intensity, without an ammunition cap.
//|---@field partial_cover boolean
// lua-types-end

// lua-types-begin btech 00220
//|---@class BattleWeaponValues
//|---@field recycle_seconds integer Effective runtime recycle time, from 1 through 127 seconds.
//|---@field battle_value integer Effective runtime Battle Value, from 0 through 2147483647.
// lua-types-end

// lua-types-begin btech 00221
//|local btech_weapon = {}
// lua-types-end

// lua-types-begin btech 00225
//|---@class BattleInventoryEntry
//|---@field part_id integer Stable game-directory part identifier.
//|---@field brand_id integer Manufacturer identifier, zero through five.
//|---@field quantity integer Positive stock quantity, at most 2147483647.
// lua-types-end

// lua-types-begin btech 00226
//|---@class BattlePart
//|---@field part_id integer Stable inventory identifier.
//|---@field name string Canonical stock name.
//|---@field kind "weapon"|"ammunition"|"component"|"commodity"|"bomb"
//|---@field mass integer Catalogue mass in 1/1024 tons; loose bomb stock uses four times this value.
// lua-types-end

// lua-types-begin btech 00227
//|local btech_inventory = {}
// lua-types-end

// lua-types-begin btech 00232
//|---@class BattleInventoryCleanup
//|---@field original_entries integer
//|---@field new_entries integer
//|---@field items integer
// lua-types-end

// lua-types-begin btech 00239
//|---@class BattleCargoRow: BattleInventoryEntry
//|---@field name string Stock display name, including a known weapon manufacturer when available.
// lua-types-end

// lua-types-begin btech 00240
//|local btech_cargo = {}
// lua-types-end

// lua-types-begin btech 00245
//|---@class btech.database
//|local btech_database = {}
// lua-types-end

// lua-types-begin btech 00246
//|---Wizard runtime diagnostics.
//|local btech_runtime = {}
// lua-types-end

// lua-types-begin btech 00247
//|---@class BattleTechInspection
//|---@field database btech.database Explicit world checkpoints.
//|---@field cargo table Cockpit stock reports and transfers.
//|---@field inventory table Shared loose-parts stock.
//|---@field weapon table Runtime weapon settings.
//|---@field character table
//|---@field template table
//|---@field map table
//|---@field player table Saved player preferences.
//|---@field unit table
//|---@field parts table Registered part catalogue and stock queries.
//|---@field repair table Immediate repair requests and technician scheduling.
//|---@field system table World event telemetry.
//|---@field autopilot BtechAutopilotAPI Lua control of unit-attached ground autopilots.
//|---@field tactical BtechTacticalAPI Filtered group observations and atomic intentions.
//|---@field errors table Structured btech error-code tree from mux.error.code_tree('btech').
//|btech = {
//|  runtime = btech_runtime,
//|  database = btech_database,
//|  cargo = btech_cargo,
//|  inventory = btech_inventory,
//|  weapon = btech_weapon,
//|  player = btech_player,
//|  character = btech_character,
//|  template = btech_template,
//|  map = btech_map,
//|  unit = btech_unit,
//|}
// lua-types-end

// lua-types-begin btech 00252
//|---@class BattleAmmunitionAdjustment
//|---@field location BattleCriticalLocation
//|---@field supplied integer Authored initial quantity.
//|---@field normalized integer Initial quantity after construction normalization.
//|---@field inferred_half_ton boolean Construction would infer a half-ton bin.
// lua-types-end

// lua-types-begin btech 00253
//|---@class BattleTemplateCheck
//|---@field chassis "biped"|"quad"|nil Parsed anatomy, independent of simulation readiness.
//|---@field name string
//|---@field reference string
//|---@field constructible boolean Passes currently implemented biped construction checks.
//|---@field rejection string|nil First construction failure; nil for a constructible template.
//|---@field weapons integer Resolved weapon count on success; zero on rejection.
//|---@field ammunition_bins integer Resolved bin count on success; zero on rejection.
//|---@field ammunition_adjustments BattleAmmunitionAdjustment[] Changes on successful construction.
// lua-types-end

// lua-types-begin btech 00255
//|---@class BattleArtemisController
//|---@field location {section: BattleSectionName|BattleVehicleSectionName, slot: integer} Zero-based controller position.
//|---@field link integer One-based template launcher slot; zero is unassigned.
//|---@field weapon_indices integer[] Zero-based missile mounts matching the link.
//|---@field operational boolean Controller is available under the unit’s equipment damage rules.
// lua-types-end

// lua-types-begin btech 00257
//|---@class BattleUnjam
//|---@field weapon_index integer Zero-based weapon number.
//|---@field remaining integer Committed seconds remaining, 1 through 60.
// lua-types-end

// lua-types-begin btech 00277
//|---@alias BattleEquipmentCondition 'empty'|'operational'|'damaged'|'disabled'|'broken'|'destroyed'|'jammed'|'shorted'|'ammo_jam'
// lua-types-end

// lua-types-begin btech 00278
//|---@class BattleWeaponDamageEffects
//|---@field moderate integer General accuracy penalty.
//|---@field ranging integer Accuracy penalty beyond short range.
//|---@field heat integer Additional firing heat.
//|---@field damage integer Energy damage reduction.
//|---@field explosion integer Nonzero count explodes on an attack roll of count plus one or less.
//|---@field jam integer Nonzero count jams on an attack roll of count plus one or less.
//|---@field feed_locked boolean Prevents changing ammunition modes.
// lua-types-end

// lua-types-begin btech 00279
//|---@class BattleWeaponDiagnostic
//|---@field index integer Zero-based installed mount number, including destroyed mounts.
//|---@field weapon string Catalogue weapon identifier (snake case).
//|---@field section string Chassis-specific location name.
//|---@field condition BattleEquipmentCondition
//|---@field damaged_slots integer
//|---@field destroyed_slots integer
//|---@field disabled_slots integer
//|---@field effects BattleWeaponDamageEffects Existing firing penalties, without recomputation in Lua.
//|---@field preferred_ammunition_section string?
// lua-types-end

// lua-types-begin btech 00281
//|---@class BattleWeaponSpecification
//|---@field weapon BattleWeapon Catalogue weapon identifier (snake case).
//|---@field ammunition BattleAmmunitionMode MMLs have separate normal (SRM) and mml_lrm rows.
//|---@field heat integer
//|---@field damage integer
//|---@field minimum_range integer
//|---@field short_range integer
//|---@field medium_range integer
//|---@field long_range integer Effective range in hexes, including artillery map-sheet conversion.
//|---@field extended_range integer? Present when extended range is configured.
//|---@field recycle_seconds integer Effective runtime value for new activations.
// lua-types-end

// lua-types-begin btech 00283
//|---@class BattleCriticalInspection
//|---@field slot integer Zero-based physical slot; native labels add one.
//|---@field equipment string Resolved display name, including configured manufacturer and bin mode.
//|---@field condition BattleEquipmentCondition
//|---@field weapon_index integer? Stable zero-based mount index, including split extensions.
//|---@field ammunition_index integer? Zero-based bin index.
//|---@field ammunition_remaining integer? Saved bin quantity; native text hides it when unavailable.
//|---@field ammunition_capacity integer? Installed bin capacity, including special rounds and half tons.
//|---@field brand integer? Authored quality; split slots use their parent weapon's brand.
//|---@field rear_mount boolean
//|---@field one_shot boolean
//|---@field spent boolean
//|---@field controls_slot integer? Authored Artemis display label, already one-based.
// lua-types-end

// lua-types-begin btech 00284
//|---@class BattleCriticalReport
//|---@field section string Stable Mech or vehicle section identity.
//|---@field name string Chassis-specific display heading.
//|---@field slots BattleCriticalInspection[] All six or twelve physical slots, including empty ones.
// lua-types-end

// lua-types-begin btech 00302
//|---@alias BattleElectronicMode "off"|"ecm"|"eccm"
//|---@class BattleElectronicField
//|---@field protected boolean
//|---@field angel_protected boolean
//|---@field disturbed boolean
//|---@field angel_disturbed boolean
//|---@field countered boolean
//|---@class BattleElectronics
//|---@field guardian BattleElectronicMode
//|---@field angel BattleElectronicMode
//|---@field field BattleElectronicField
// lua-types-end

// lua-types-begin btech 00308
//|---@class BattlePodRow
//|---@field section BattleSection|BattleVehicleSectionName
//|---@field destroyed boolean
//|---@field kinds BattleBeaconKind[]
//|---@class BattlePodRemoval
//|---@field section BattleSection
//|---@field kind BattleBeaconKind
//|---@field arm "left"|"right"
//|---@field target_number integer
//|---@field roll integer
//|---@field removed boolean
//|---@field self_damage integer
//|---@field impact BattleTacticalImpact|nil
//|---@field notices BattleNotice[]
// lua-types-end

// lua-types-begin btech 00312
//|---@class BattleSignatureTransition
//|---@field enabled boolean
//|---@field remaining integer
//|---@class BattleSignatureState
//|---@field enabled boolean
//|---@field pending BattleSignatureTransition|nil
// lua-types-end

// lua-types-begin btech 00318
//|---@class BattleHexLock
//|---@field hex {x: integer, y: integer}
//|---@field mode 'unit_at_hex'|'hex'|'building'|'ignite'|'clear'
//|---@field remaining integer Eight seconds to settle; zero is settled.
// lua-types-end

// lua-types-begin btech 00320
//|---@class BattleHexAimModifiers: BattleAimModifiers
//|---@field hex {x: integer, y: integer}
//|---@field mode 'unit_at_hex'|'hex'|'building'|'ignite'|'clear'
//|---@field visible boolean Current terrain visibility, separate from numeric aim.
//|---@field hex_bonus integer Zero for unit-at-hex, otherwise -4.
//|---@field subtotal integer|nil Nil beyond weapon range; numeric aim alone does not authorize firing.
// lua-types-end

// lua-types-begin btech 00325
//|---@class BattleRadioReception
//|---@field receiver integer
//|---@field channel integer Zero-based receiving channel.
//|---@field transmitters integer[] Sender and any relays, excluding receiver.
//|---@field bearing integer Bearing toward final transmitter.
//|---@field text string Formatted cockpit message.
// lua-types-end

// lua-types-begin btech 00326
//|---@class BattleChannelMessage
//|---@field channel "debug"|"economy"|"attack_experience"|"experience"|"piloting_experience"|"frequencies"|"zero_frequencies"|"map_errors"
//|---@field text string
// lua-types-end

// lua-types-begin btech 00327
//|---@class BattleRadioTransmission
//|---@field delivery {mode: 'analog'|'digital', report: {sender: integer, map: integer, frequency: integer, receptions: BattleRadioReception[], interfered_receivers: integer[]?, scans: {receiver: integer, channel: integer, previous: integer, frequency: integer}[]?, notifications: BattleNotice[]?}}
//|---@field mines table Ordered frequency-matched command-mine report and consequences.
//|---@field audit_messages BattleChannelMessage[] Diagnostics committed with the transmission.
//|---@field experience_messages BattleChannelMessage[] Accepted communication XP diagnostics.
// lua-types-end

// lua-types-begin btech 00329
//|---@class BattleTargetedRadioReport
//|---@field sender integer
//|---@field target integer
//|---@field notices {unit: integer, text: string}[] Captured sender echo and powered-recipient message.
// lua-types-end

// lua-types-begin btech 00333
//|---@class BattleBuildingScan
//|---@field text string Cockpit reply; undiscovered and missing buildings share one message.
//|---@field experience_messages BattleChannelMessage[] Accepted perception diagnostics.
// lua-types-end

// lua-types-begin btech 00335
//|---@class BattleMineScan
//|---@field found boolean Successful recognition only; configuration is never disclosed.
//|---@field text string
//|---@field experience_messages BattleChannelMessage[]
// lua-types-end

// lua-types-begin btech 00336
//|---@class BattleHexScan
//|---@field building BattleBuildingScan
//|---@field mines BattleMineScan
// lua-types-end

// lua-types-begin btech 00338
//|---@class BattleSelectedScan
//|---@field kind 'unit'|'building'|'hex'
//|---@field report string|BattleBuildingScan|BattleHexScan
// lua-types-end

// lua-types-begin btech 00341
//|---@class BattleViewPosition
//|---@field map integer Scanner battlefield dbref.
//|---@field center {x: integer, y: integer} Requested center before viewport clipping.
//|---@field maximum_range integer Damage-adjusted display hardware radius.
// lua-types-end

// lua-types-begin btech 00343
//|---@class BattleViewDimensions
//|---@field tactical_width integer? Requested columns, 5..40; default 21.
//|---@field tactical_height integer? Requested rows, 5..24; default 14.
//|---@field long_range_height integer? Requested rows, 10..40; default 11.
// lua-types-end

// lua-types-begin btech 00344
//|---@class BattleViewport
//|---@field map integer
//|---@field requested_center {x: integer, y: integer}
//|---@field origin {x: integer, y: integer} Upper-left in-bounds coordinate.
//|---@field width integer Column count.
//|---@field height integer Row count.
//|---@field maximum_range integer
// lua-types-end

// lua-types-begin btech 00346
//|---@class BattleLongRangeMap
//|---@field viewport BattleViewport
//|---@field text string Filtered staggered-row display.
// lua-types-end

// lua-types-begin btech 00348
//|---@class BattleTacticalMap
//|---@field viewport BattleViewport
//|---@field text string Styled hex display with acquired two-character contact labels.
// lua-types-end

// lua-types-begin btech 00350
//|---@class BattleHexCenterReport
//|---@field coordinate BattleHexCoordinate
//|---@field elevation integer
//|---@field range number Horizontal range to the current hex center.
//|---@field bearing integer Clockwise degrees; 180 at the exact center.
//|---@field text string Shared native readout.
// lua-types-end

// lua-types-begin btech 00352
//|---@class BattleNavigationReport
//|---@field center BattleHexCoordinate Requested local map center.
//|---@field text string Styled local map, continuous-position plot and live readouts.
// lua-types-end

// lua-types-begin btech 00355
//|---@alias BattleBuildingContactMode "follow_brief" | "include" | "exclude"
// lua-types-end

// lua-types-begin btech 00356
//|---@class BattleContactPreferences
//|---@field include_dead boolean Defaults false.
//|---@field include_shutdown boolean Defaults true.
//|---@field include_enemies boolean Defaults true.
//|---@field include_allies boolean Defaults true.
//|---@field include_target boolean Defaults true; never bypasses visibility.
//|---@field buildings BattleBuildingContactMode Defaults "exclude"; applies to native contacts +, independently of unit filtering.
// lua-types-end

// lua-types-begin btech 00358
//|---@class BattleContactOptions
//|---@field buildings boolean Include building contacts for native output.
//|---@field preferences BattleContactPreferences Decoded unit categories.
//|---@field ignored string[] Unrecognized characters in encounter order.
// lua-types-end

// lua-types-begin btech 00360
//|---@class BattleBuildingContact
//|---@field detection BattleDetectionChannel|nil Whether the sensor band or sight reaches the entrance.
//|---@field short_text string Plain compact row after identification locks.
//|---@field weapon_arc BattleContactArc Observer torso direction toward entrance.
//|---@field interior integer
//|---@field coordinate BattleHexCoordinate
//|---@field elevation integer
//|---@field name string Plain structure name.
//|---@field range number
//|---@field bearing integer
//|---@field integrity integer
//|---@field maximum_integrity integer
//|---@field identified boolean Identification lock result.
//|---@field hidden boolean Concealed entrance identified successfully.
//|---@field status string Blank, x (restricted), X (safe/restricted command center), or C (command center).
// lua-types-end

// lua-types-begin btech 00362
//|---@class BattleBriefSettings
//|---@field contacts integer Contact mode 0..3; defaults 1.
//|---@field automatic integer Routine notice mode 0..6; defaults 0.
// lua-types-end

// lua-types-begin btech 00363
//|---@class BattleBriefReport
//|---@field settings BattleBriefSettings
//|---@field changed boolean An edit was requested; query is false.
//|---@field text string Query or cockpit confirmation.
// lua-types-end

// lua-types-begin btech 00367
//|---@class BattleBootleggerReport
//|---@field modifier integer Situational difficulty and failed-fall severity.
//|---@field check BattlePilotingCheck
//|---@field fall BattleFallReport?
//|---@field notices BattleNotice[]
// lua-types-end

// lua-types-begin btech 00369
//|---@class BattleEtaReport
//|---@field coordinate BattleHexCoordinate
//|---@field range number Horizontal range.
//|---@field minutes integer? Whole minutes, absent when effectively stationary.
//|---@field text string
// lua-types-end

// lua-types-begin btech 00371
//|---@class BattleBearingReport
//|---@field origin BattlePoint
//|---@field destination BattlePoint
//|---@field bearing integer Clockwise compass degrees, 180 for coincident points.
//|---@field text string
// lua-types-end

// lua-types-begin btech 00373
//|---@class BattleRangeReport
//|---@field horizontal number Horizontal distance in hexes.
//|---@field spatial number Spatial distance after dark-map terrain masking.
//|---@field text string
// lua-types-end

// lua-types-begin btech 00375
//|---@class BattleVectorReport
//|---@field horizontal number
//|---@field spatial number
//|---@field bearing integer Clockwise compass bearing.
//|---@field vertical_bearing integer Signed vertical angle rounded away from zero.
//|---@field text string
// lua-types-end

// lua-types-begin btech 00378
//|---@class BattleBoosterState
//|---@field enabled boolean
//|---@field counter integer
//|---@field remaining integer Seconds until the next overload/recovery check.
//|---@field failed boolean Hardware failure persists through shutdown.
// lua-types-end

// lua-types-begin btech 00383
//|---@class BattleNetworkStatusRow
//|---@field unit integer
//|---@field label string
//|---@field name string
//|---@field coordinate BattleHexCoordinate
//|---@field elevation integer
//|---@field range number
//|---@field bearing integer
//|---@field speed number
//|---@field heading integer
//|---@field armor_percent integer
//|---@field internal_percent integer
// lua-types-end

// lua-types-begin btech 00406
//|---@class BattleSwarmHop
//|---@field target integer
//|---@field incoming integer
//|---@field roll integer
//|---@field remaining integer
//|---@field salvo {kind: 'mech'|'vehicle', report: table}|nil Absent for a secondary miss.
// lua-types-end

// lua-types-begin btech 00407
//|---@class BattleSwarmReport
//|---@field launched integer
//|---@field remaining integer
//|---@field traveled number Cumulative distance, including a terminal leg that falls short.
//|---@field hops BattleSwarmHop[] Ordered attacks, at most eleven.
//|---@field notices BattleNotice[]
//|---@field broadcasts BattleNotice[]
// lua-types-end

// lua-types-begin btech 00411
//|---@class BattleWeaponDamage
//|---@field location {section: BattleSectionName, slot: integer}
//|---@field effects ("moderate"|"focus"|"crystal"|"ranging"|"barrel"|"feed")[] Distinct component effects; empty means superficial damage.
// lua-types-end

// lua-types-begin btech 00417
//|---@class BattleRuntimeStats
//|---@field simulation_pending boolean Same work predicate as the server's one-second simulation tick.
//|---@field scanner_observers integer
//|---@field reactor_startup_remaining integer
//|---@field artillery_shots integer
//|---@field maps integer
//|---@field mechs integer
//|---@field vehicles integer
//|---@field registration_kinds table<string, integer>
//|---@field inline_record_bytes integer Root/map/unit inline sizes only; heap storage excluded.
//|---@field encoded_state_bytes integer Exact compact JSON encoding size, not allocator usage.
// lua-types-end

// lua-types-begin btech 00420
//|---@class BattleUnitField
//|---@field name string Full field name, independent of display width.
//|---@field value string|nil Available field value; nil displays as n/a.
// lua-types-end

// lua-types-begin btech 00421
//|---@class BattleUnitFieldReport
//|---@field unit integer
//|---@field columns integer
//|---@field fields BattleUnitField[]
//|---@field text string Literal report text, already published to the actor.
// lua-types-end

// lua-types-begin btech 00424
//|-- C-parity contract surface shared by several groups below.
// lua-types-end

// lua-types-begin btech 00425
//|---Typed unit-layout section constant from btech.unit.sections.
//|---@class BattleSection
//|---Typed unit class constant from btech.unit.types.
//|---@class BattleUnitType
//|---Typed movement class constant from btech.unit.movement_types.
//|---@class BattleMovementType
//|---Typed technology code from btech.unit.technology.
//|---@class BattleTechnologyCode
//|---Typed technology group from btech.unit.technology_groups.
//|---@class BattleTechnologyGroup
//|---Typed weapon fire-mode constant from btech.unit.fire_modes.
//|---@class BattleFireModeConstant
//|---Typed ammunition-mode constant from btech.unit.ammunition_modes.
//|---@class BattleAmmunitionModeConstant
//|---Typed repair operation from btech.repair.operations.
//|---@class BattleRepairOperation
//|---Typed battlefield light constant from btech.map.light_levels.
//|---@class BattleLightLevel
//|---Typed searchlight switching policy from btech.unit.searchlight_modes.
//|---@class BattleSearchlightMode
// lua-types-end

// lua-types-begin btech 00426
//|---@class BattleValuePair
//|---@field current integer
//|---@field original integer
// lua-types-end

// lua-types-begin btech 00427
//|---@class BattleArmorStatus
//|---@field section? BattleSection Omitted when the request did not select one.
//|---@field armor BattleValuePair
//|---@field internal BattleValuePair
//|---@field rear_armor BattleValuePair
// lua-types-end

// lua-types-begin btech 00428
//|---@class BattleAmmunitionStatus
//|---@field rounds integer
//|---@field capacity integer
// lua-types-end

// lua-types-begin btech 00429
//|---@class BattleWeaponStats
//|---@field kind string
//|---@field heat integer
//|---@field damage integer
//|---@field minimum_range integer
//|---@field short_range integer
//|---@field medium_range integer
//|---@field long_range integer
//|---@field critical_slots integer
//|---@field ammunition_per_ton integer
//|---@field recycle_time integer
//|---@field battle_value integer
// lua-types-end

// lua-types-begin btech 00430
//|---@class BattlePartDefinition
//|---@field id integer Stable catalogue part identifier.
//|---@field brand integer Manufacturer identifier.
//|---@field packed_id integer Brand-major combined identifier.
//|---@field short_name string
//|---@field long_name string
//|---@field very_long_name string
//|---@field category string
//|---@field weight_tons number
//|---@field cost integer
//|---@field weapon? BattleWeaponStats Present for weapon parts.
// lua-types-end

// lua-types-begin btech 00431
//|---@alias BattlePartRef BattlePartDefinition|integer|string
// lua-types-end

// lua-types-begin btech 00432
//|---@class BattlePartStack
//|---@field part BattlePartDefinition
//|---@field quantity integer
// lua-types-end

// lua-types-begin btech 00433
//|---@class BattlePartCategory
//|---@field code string
//|---@field name string
// lua-types-end

// lua-types-begin btech 00434
//|---@class BattleCriticalSlot
//|---@field section BattleSection
//|---@field slot integer
//|---@field kind string
//|---@field part? BattlePartDefinition
//|---@field operational boolean
//|---@field temporary_failure boolean
//|---@field auxiliary_data integer
//|---@field ammunition? BattleAmmunitionStatus
//|---@field fire_modes BattleFireModeConstant[]
//|---@field ammunition_modes BattleAmmunitionModeConstant[]
// lua-types-end

// lua-types-begin btech 00435
//|---@class BattleMountedWeapon
//|---@field number integer Zero-based stable weapon number.
//|---@field section BattleSection
//|---@field first_slot integer Zero-based first occupied critical slot.
//|---@field part BattlePartDefinition
//|---@field slot_count integer
//|---@field recycle integer Seconds remaining in the current cycle.
//|---@field recycle_time integer Full recycle time in seconds.
//|---@field operational boolean
// lua-types-end

// lua-types-begin btech 00436
//|---@class BattleEngine
//|---@field rating integer
//|---@field suspension_factor integer
// lua-types-end

// lua-types-begin btech 00437
//|---@class BattleRadioChannelReport
//|---@field channel integer One-based channel position.
//|---@field frequency integer Frequency from 0 through 999999.
//|---@field title string At most fifteen UTF-8 bytes.
//|---@field modes string[] Active mode names: digital, mute, relay, information, scan.
// lua-types-end

// lua-types-begin btech 00438
//|---@class BattleBattleValue
//|---@field total number
//|---@field offensive number
//|---@field defensive number
// lua-types-end

// lua-types-begin btech 00439
//|---@class BattleTechnology
//|---@field code BattleTechnologyCode
//|---@field name string
//|---@field group "primary"|"secondary"|"infantry"
//|---@field source "configured"|"inferred"
// lua-types-end

// lua-types-begin btech 00440
//|-- C-parity character value and progress contracts.
// lua-types-end

// lua-types-begin btech 00441
//|---@class BattleCharacterValueDefinition
//|---@field code integer
//|---@field name string
//|---@field kind string Char_value, Char_skill, Char_advantage or Char_attribute.
//|---@field default_experience_threshold integer
// lua-types-end

// lua-types-begin btech 00442
//|---@class BattleCharacterValueReport
//|---@field definition BattleCharacterValueDefinition
//|---@field amount integer
//|---@field target? integer Skill targets including earned levels.
//|---@field experience? integer
//|---@field experience_to_next_level? integer
// lua-types-end

// lua-types-begin btech 00450
//|-- C-parity personal-combat and player-preference contracts.
// lua-types-end

// lua-types-begin btech 00451
//|---@class BattlePersonalCombatArmor
//|---@field head integer
//|---@field torso integer
//|---@field hands integer
//|---@field feet integer
// lua-types-end

// lua-types-begin btech 00452
//|---@class BattlePersonalCombatEquipment
//|---@field weapon BattlePartDefinition
//|---@field ammunition? integer
// lua-types-end

// lua-types-begin btech 00453
//|---@class BattlePersonalCombatLoadout
//|---@field armor BattlePersonalCombatArmor
//|---@field right? BattlePersonalCombatEquipment
//|---@field left? BattlePersonalCombatEquipment
// lua-types-end

// lua-types-begin btech 00454
//|---@class BattleUiPreferencesState
//|---@field tactical_height integer
//|---@field tactical_width integer
//|---@field lrs_height integer
//|---@field include_dead boolean
//|---@field include_shutdown boolean
//|---@field include_enemies boolean
//|---@field include_allies boolean
//|---@field include_target boolean
//|---@field buildings "follow_brief"|"include"|"exclude"
//|---@field configured boolean
// lua-types-end

// lua-types-begin btech 00461
//|-- C-parity template inspection contracts.
// lua-types-end

// lua-types-begin btech 00475
//|-- C-parity unit inspection and administration contracts.
// lua-types-end

// lua-types-begin btech 00497
//|---@class BattleWeaponInstall
//|---@field part BattlePartRef Weapon part reference.
//|---@field section BattleSection
//|---@field slots integer[] Zero-based critical slots.
//|---@field rear_facing? boolean
//|---@field targeting_computer? boolean
//|---@field one_shot? boolean
// lua-types-end

// lua-types-begin btech 00498
//|---@class BattleAmmunitionConfiguration
//|---@field weapon BattlePartRef Launcher part reference.
//|---@field section BattleSection
//|---@field slot integer Zero-based critical slot.
//|---@field half_ton? boolean
//|---@field ammunition_modes? BattleAmmunitionModeConstant[]
// lua-types-end

// lua-types-begin btech 00499
//|---@class BattleWeaponModes
//|---@field fire_modes? BattleFireModeConstant[]
//|---@field ammunition_modes? BattleAmmunitionModeConstant[]
// lua-types-end

// lua-types-begin btech 00500
//|---@class BattleSpecialInstall
//|---@field part? BattlePartRef Omit to empty the slot.
//|---@field section BattleSection
//|---@field slot integer Zero-based critical slot.
//|---@field auxiliary_data? integer
// lua-types-end

// lua-types-begin btech 00523
//|-- C-parity part catalogue contracts.
// lua-types-end

// lua-types-begin btech 00524
//|local btech_parts = {}
// lua-types-end

// lua-types-begin btech 00533
//|-- C-parity repair contracts.
// lua-types-end

// lua-types-begin btech 00534
//|local btech_repair = {}
// lua-types-end

// lua-types-begin btech 00535
//|---@class BattleRepairArmorRequest
//|---@field operation BattleRepairOperation
//|---@field section BattleSection
//|---@field value integer
//|---@class BattleRepairInternalRequest
//|---@field operation BattleRepairOperation
//|---@field section BattleSection
//|---@field value integer
//|---@class BattleRepairRearArmorRequest
//|---@field operation BattleRepairOperation
//|---@field section BattleSection
//|---@field value integer
//|---@class BattleRepairPartRequest
//|---@field operation BattleRepairOperation
//|---@field section BattleSection
//|---@field slot integer
//|---@class BattleRepairReattachRequest
//|---@field operation BattleRepairOperation
//|---@field section BattleSection
// lua-types-end

// lua-types-begin btech 00536
//|---@alias BattleImmediateRepair BattleRepairArmorRequest|BattleRepairInternalRequest|BattleRepairRearArmorRequest|BattleRepairPartRequest|BattleRepairReattachRequest
// lua-types-end

// lua-types-begin btech 00540
//|-- C-parity world telemetry contracts.
// lua-types-end

// lua-types-begin btech 00541
//|local btech_system = {}
// lua-types-end

// lua-types-begin btech 00544
//|-- Typed autopilot constants and unit-attached controller operations.
// lua-types-end

// lua-types-begin btech 00545
//|---@class BattleAutopilotOrderName
//|---@class BattleAutopilotSubmissionMode
//|---@class BattleAutopilotFireMode
//|---@class BattleAutopilotStates
//|---@field PAUSED "paused"
//|---@field IDLE "idle"
//|---@field EXECUTING "executing"
//|---@field BLOCKED "blocked"
//|---@class BattleAutopilotOrderStates
//|---@field QUEUED "queued"
//|---@field RUNNING "running"
//|---@field SUCCEEDED "succeeded"
//|---@field FAILED "failed"
//|---@field CANCELED "canceled"
//|---@class BattleAutopilotReasons
//|---@field MANUAL_TAKEOVER "manual_takeover"
//|---@field CONTACT_LOST "contact_lost"
//|---@field STUCK "stuck"
//|---@field UNREACHABLE "unreachable"
//|---@field INVALIDATED "invalidated"
//|---@field RESOURCE_LIMIT "resource_limit"
//|---@field CONGESTED "congested"
//|---@field INVALID_TARGET "invalid_target"
//|---@field UNIT_UNAVAILABLE "unit_unavailable"
//|---@field MAP_CHANGED "map_changed"
//|---@field UNSUPPORTED "unsupported"
//|---@field STALE_REVISION "stale_revision"
//|---@class BattleAutopilotRangeBand
//|---@field minimum integer Inclusive minimum engagement range in hexes.
//|---@field maximum integer Inclusive maximum engagement range in hexes.
//|---@class BattleAutopilotConfig
//|---@field speed_percent integer Desired speed as a percentage from 0 through 100.
//|---@field fire_mode "hold"|"assigned_target"|"opportunistic" Current serialized weapon policy.
//|---@field heat_ceiling integer Projected heat limit for autonomous fire.
//|---@field preferred_range BattleAutopilotRangeBand|nil Optional engagement band.
//|---@class BattleAutopilotConfigPatch
//|---@field speed_percent integer|nil Optional speed update.
//|---@field fire_mode BattleAutopilotFireMode|nil Optional weapon-policy update.
//|---@field heat_ceiling integer|nil Optional projected heat limit.
//|---@field preferred_range BattleAutopilotRangeBand|false|nil Set or clear the preferred band.
//|---@alias BattleAutopilotControllerState "paused"|"idle"|"executing"|"blocked"
//|---@alias BattleAutopilotOrderState "queued"|"running"|"succeeded"|"failed"|"canceled"
//|---@alias BattleAutopilotFeedbackEvent "configured"|"paused"|"resumed"|"manual_takeover"|"order_queued"|"order_started"|"order_succeeded"|"order_failed"|"order_canceled"|"blocked"
//|---@alias BattleAutopilotReason "manual_takeover"|"contact_lost"|"stuck"|"unreachable"|"invalidated"|"resource_limit"|"congested"|"invalid_target"|"unit_unavailable"|"map_changed"|"unsupported"|"stale_revision"
//|---@class BattleAutopilotOrder
//|---@field kind BattleAutopilotOrderName
//|---@field destination BattlePosition|nil Move or attack-move destination.
//|---@field arrival_radius integer|nil Destination tolerance in hexes.
//|---@field target integer|nil Follow or attack target unit.
//|---@field separation integer|nil Follow distance in hexes.
//|---@field waypoints BattlePosition[]|nil Patrol route.
//|---@field range BattleAutopilotRangeBand|nil Optional attack engagement band.
//|---@class BattleAutopilotStoredOrder
//|---@field kind "move"|"hold"|"follow"|"patrol"|"attack"|"attack_move" Serialized intent kind.
//|---@field destination BattlePosition|nil Move or attack-move destination.
//|---@field arrival_radius integer|nil Destination tolerance in hexes.
//|---@field target integer|nil Follow or attack target unit.
//|---@field separation integer|nil Follow distance in hexes.
//|---@field waypoints BattlePosition[]|nil Patrol route.
//|---@field range BattleAutopilotRangeBand|nil Optional engagement band.
//|---@class BattleAutopilotOrderProgress
//|---@field waypoint_index integer Current waypoint cursor.
//|---@field recovery_attempts integer Replanning attempts for the active order.
//|---@field stagnant_ticks integer Ticks without route progress.
//|---@field attack_move_origin BattlePosition|nil Position where attack-move pursuit began.
//|---@field attack_move_suppressed_target integer|nil Contact already engaged during attack-move.
//|---@class BattleAutopilotOrderRecord
//|---@field id integer Stable controller-local order ID.
//|---@field order BattleAutopilotStoredOrder Serialized order intent; submissions use typed constants.
//|---@field state BattleAutopilotOrderState Lifecycle state.
//|---@field progress BattleAutopilotOrderProgress Durable execution cursor.
//|---@class BattleAutopilotStatus
//|---@field config BattleAutopilotConfig Controller settings.
//|---@field state BattleAutopilotControllerState Controller lifecycle state.
//|---@field blocking_reason BattleAutopilotReason|nil Reason the controller is blocked, if any.
//|---@field revision integer Management revision.
//|---@field next_order_id integer Next order ID that will be assigned.
//|---@field active BattleAutopilotOrderRecord|nil Current order.
//|---@field queue BattleAutopilotOrderRecord[] Queued orders.
//|---@field feedback BattleAutopilotFeedback[] Recently retained outcomes.
//|---@field next_feedback_sequence integer Next feedback sequence that will be assigned.
//|---@field sightings table<integer, BattleAutopilotSighting> Retained contact memory keyed by unit ID.
//|---@class BattleAutopilotSubmitResult
//|---@field ids integer[] Assigned order IDs.
//|---@field revision integer New management revision.
//|---@class BattleAutopilotContact
//|---@field unit integer Acquired unit identity.
//|---@field position BattlePosition Observed position.
//|---@field friendly boolean Whether the contact is allied.
//|---@field identified boolean Whether sensors identified the contact well enough to determine allegiance.
//|---@field known_destroyed boolean Whether the visible contact status reports destruction.
//|---@field range number Observed range in map units.
//|---@field network_range number|nil Shared C3/C3i aiming distance; nil without an active network.
//|---@field relayed boolean Seen only by network peers; the unit cannot lock or fire on it yet.
//|---@field seen_at integer Simulation time of the observation.
//|---@class BattleAutopilotMemory
//|---@field unit integer Previously acquired unit identity.
//|---@field position BattlePosition Last sensor-confirmed position.
//|---@field seen_at integer Simulation time of the last sighting.
//|---@class BattleHeat
//|---@field stored number Current stored weapon heat.
//|---@field excess number Sampled excess heat.
//|---@class BattleAutopilotOwnReadiness
//|---@field power BattlePower Current power state.
//|---@field maximum_speed number Damage-adjusted maximum speed.
//|---@field heat BattleHeat|nil Conventional heat state; nil for ground vehicles.
//|---@field weapons BattleWeaponReadiness[] Readiness for installed weapons.
//|---@class BattleAutopilotSighting
//|---@field position BattlePosition Last sensor-confirmed position.
//|---@field seen_at integer Simulation time of the last sighting.
//|---@class BattleAutopilotObservation
//|---@field unit integer Observing unit.
//|---@field time integer Current simulation time.
//|---@field position BattlePosition|nil Own position, if placed.
//|---@field heading number|nil Own heading, if motion is available.
//|---@field speed number Own current speed.
//|---@field own BattleAutopilotOwnReadiness Own mechanical and weapon readiness.
//|---@field contacts BattleAutopilotContact[] Current sensor contacts, plus those relayed by active C3/C3i peers.
//|---@field remembered BattleAutopilotMemory[] Fresh retained sightings.
//|---@class BattleAutopilotFeedback
//|---@field sequence integer Monotonic feedback sequence.
//|---@field simulation_time integer Simulation time of the event.
//|---@field order_id integer|nil Related order ID.
//|---@field event BattleAutopilotFeedbackEvent Event kind.
//|---@field reason BattleAutopilotReason|nil Optional event reason.
//|---@class BattleAutopilotFeedbackPage
//|---@field records BattleAutopilotFeedback[] Retained feedback records after the cursor.
//|---@field history_gap boolean Whether older records fell outside the retention window.
//|---@class BtechAutopilotAPI
//|---@field orders table
//|---@field submission_modes table
//|---@field fire_modes table
//|---@field states BattleAutopilotStates Controller lifecycle strings.
//|---@field order_states BattleAutopilotOrderStates Order lifecycle strings.
//|---@field reasons BattleAutopilotReasons Blocking and outcome reason strings.
//|---@field attach fun(unit: integer, options?: BattleAutopilotConfigPatch)
//|---@field detach fun(unit: integer)
//|---@field configure fun(unit: integer, patch: BattleAutopilotConfigPatch, expected_revision?: integer): integer
//|---@field submit fun(unit: integer, orders: BattleAutopilotOrder[], mode: BattleAutopilotSubmissionMode, expected_revision?: integer): BattleAutopilotSubmitResult
//|---@field cancel fun(unit: integer, order_id: integer, expected_revision?: integer): boolean
//|---@field pause fun(unit: integer)
//|---@field resume fun(unit: integer)
//|---@field status fun(unit: integer): BattleAutopilotStatus
//|---@field observe fun(unit: integer): BattleAutopilotObservation
//|---@field feedback fun(unit: integer, after_sequence?: integer): BattleAutopilotFeedbackPage
// lua-types-end

// lua-types-begin btech 00546
//|local btech_autopilot = {} ---@type BtechAutopilotAPI
// lua-types-end

// lua-types-begin btech 00547
//|btech.parts = btech_parts
//|btech.repair = btech_repair
//|btech.system = btech_system
//|btech.autopilot = btech_autopilot
// lua-types-end

// lua-types-begin btech 00548
//|---@class BattleTacticalUnitSnapshot
//|---@field unit integer Assigned friendly unit ID.
//|---@field revision integer Management revision used for stale-intention protection.
//|---@field status BattleAutopilotStatus Controller state; sightings are supplied through observation instead.
//|---@field observation BattleAutopilotObservation Per-unit permitted intelligence.
//|---@field feedback BattleAutopilotFeedbackPage Outcome page after the requested cursor.
//|---@class BattleTacticalSighting
//|---@field observer integer Unit that acquired this sighting.
//|---@field position BattlePosition Last observed position.
//|---@field seen_at integer Committed simulation seconds.
//|---@field current boolean Whether this observer currently acquires the contact.
//|---@field friendly boolean|nil Present only for a current observation.
//|---@field identified boolean|nil Present only for a current observation.
//|---@field known_destroyed boolean|nil Present only for a current observation.
//|---@field relayed boolean|nil Present only for a current observation; true when only C3/C3i peers see it.
//|---@class BattleTacticalContact
//|---@field unit integer Contact identity.
//|---@field observations BattleTacticalSighting[] Source observations, ordered by observer ID.
//|---@class BattleTacticalSnapshot
//|---@field version integer Snapshot schema version, currently 1.
//|---@field time integer Committed simulation seconds; restart does not advance this clock.
//|---@field units BattleTacticalUnitSnapshot[] Assigned controllers, ordered by unit ID.
//|---@field contacts BattleTacticalContact[] Aggregated sightings, ordered by contact ID.
//|---@class BattleTacticalIntention
//|---@field unit integer Assigned unit ID.
//|---@field expected_revision integer Required current management revision.
//|---@field mode BattleAutopilotSubmissionMode Append or replace using typed constants.
//|---@field orders BattleAutopilotOrder[] Ordinary unit orders, at most 64.
//|---@class BattleTacticalSubmitResult : BattleAutopilotSubmitResult
//|---@field unit integer Controller receiving these order IDs.
//|---@class BtechTacticalAPI
//|local btech_tactical = {}
//|---Read a detached tactical snapshot for 1 to 100 distinct friendly controllers on one map.
//|---Shared sightings retain observer provenance and do not grant another unit attack admission.
//|---@param units integer[] Explicit assigned unit IDs; every unit must be attached and placed.
//|---@param feedback_cursors table<integer, integer>|nil Optional per-unit feedback sequence cursors.
//|---@return BattleTacticalSnapshot snapshot Versioned intelligence and controller outcomes.
//|function btech_tactical.observe(units, feedback_cursors) end
//|---Atomically validate and submit intentions for 1 to 100 distinct friendly controllers.
//|---Any invalid order or stale revision rejects the whole batch. Paused units stay paused.
//|---@param intentions BattleTacticalIntention[] One intention per unit; revisions are required.
//|---@return BattleTacticalSubmitResult[] results Assigned IDs and revisions in request order.
//|function btech_tactical.submit(intentions) end
//|btech.tactical = btech_tactical
// lua-types-end

// lua-types-begin btech 00549
//|return btech
// lua-types-end
