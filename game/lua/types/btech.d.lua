---@meta _

---Generated from Rust LuaLS contracts; run `just update-lua-types`.

---BattleTech inspection and supported tactical control APIs implemented by the Rust runtime.
---Results are detached values, available only inside game callbacks.

---@alias BattleSectionName "LeftArm"|"RightArm"|"LeftTorso"|"RightTorso"|"CenterTorso"|"LeftLeg"|"RightLeg"|"Head"

---@class BattleNotice
---@field unit integer Recipient unit dbref.
---@field text string Cockpit message text.

---@class BattleCriticalDefinition
---@field equipment string Unresolved asset equipment name.
---@field data string Unresolved asset data token.
---@field modes string[] Unresolved asset mode names.
---@field brand integer|nil Optional asset brand.

---@class BattleSectionDefinition
---@field armor integer
---@field internal integer
---@field rear integer
---@field criticals table<integer, BattleCriticalDefinition> Zero-based slot positions.
---@field configuration string|nil

---@class BattleTemplate
---@field name string
---@field reference string
---@field tons integer
---@field max_speed number
---@field jump_speed number
---@field heat_sinks integer Cooling capacity; double sinks are already counted twice.
---@field sections table<BattleSectionName, BattleSectionDefinition>
---@field attributes table<string, string> Unit-level source fields; not validated simulation capabilities.

---@class BattleMapAssetSummary
---@field width integer
---@field height integer
---@field gravity integer
---@field temperature integer
---@field flags integer

---@class BattleHex
---@field terrain string Snake_case terrain name.
---@field elevation integer Magnitude from 0 through 9; water and ice represent depth.

---@class StoredBattleMap
---@field cargo_transfer_point BattleCargoTransferPoint|nil Saved cargo location and hint policy.
---@field wrapping boolean Opposite-edge wrapping is enabled.
---@field linked_markers table<integer, {coordinate: BattleHexCoordinate, object: integer, data_char: integer, data_short: integer, data_int: integer}> Complete authored linked marker records.
---@field building_exits table<integer, {coordinate: BattleHexCoordinate, destination: integer, data_char: integer, data_short: integer, data_int: integer}> Return-link slots; coordinates are selection metadata.
---@field name string
---@field width integer
---@field height integer
---@field gravity integer
---@field temperature integer

---@field flags integer
---@field light integer 0 night, 1 twilight, 2 day
---@field visibility integer Weather range in hexes
---@field sensor_flags integer Disabled perception channels: sensor band bit 0 (1), radar bit 5 (32), probes bit 6 (64).
---@field maximum_visibility integer Saved map sensor range ceiling
---@field terrain_ready boolean Whether saved tiles have a valid dictionary.

---@class StoredBattleUnit
---@field name string
---@field template string
---@field class_code integer Saved unit class, including deferred classes.
---@field movement_code integer Saved movement type, including deferred types.
---@field tons integer
---@field map integer|nil

---@alias BattleWeapon "arrow_iv"|"clan_arrow_iv"|"long_tom"|"sniper"|"thumper"|"long_tom_cannon"|"sniper_cannon"|"thumper_cannon"|"a_pod"|"clan_a_pod"|"i_narc_beacon"|"narc_beacon"|"clan_narc_beacon"|"anti_missile_system"|"clan_anti_missile_system"|"laser_ams"|"clan_laser_ams"|"clan_lbx2"|"clan_lbx5"|"clan_lbx10"|"clan_lbx20"|"clan_ultra_ac2"|"clan_ultra_ac5"|"clan_ultra_ac10"|"clan_ultra_ac20"|"mml3"|"mml5"|"mml7"|"mml9"|"clan_atm3"|"clan_atm6"|"clan_atm9"|"clan_atm12"|"clan_lrm5"|"clan_lrm10"|"clan_lrm15"|"clan_lrm20"|"clan_srm2"|"clan_srm4"|"clan_srm6"|"clan_streak_srm2"|"clan_streak_srm4"|"clan_streak_srm6"|"clan_streak_lrm5"|"clan_streak_lrm10"|"clan_streak_lrm15"|"clan_streak_lrm20"|"clan_gauss_rifle"|"clan_machine_gun"|"clan_light_machine_gun"|"clan_heavy_machine_gun"|"clan_er_large_laser"|"clan_er_medium_laser"|"clan_er_small_laser"|"clan_er_micro_laser"|"clan_er_ppc"|"clan_flamer"|"clan_heavy_large_laser"|"clan_heavy_medium_laser"|"clan_heavy_small_laser"|"clan_large_pulse_laser"|"clan_medium_pulse_laser"|"clan_small_pulse_laser"|"clan_micro_pulse_laser"|"clan_er_large_pulse_laser"|"clan_er_medium_pulse_laser"|"clan_er_small_pulse_laser"|"clan_plasma_rifle"|"flamer"|"coolant_gun"|"heavy_flamer"|"vehicle_flamer"|"vehicle_heavy_flamer"|"plasma_rifle"|"acid_thrower"|"thunderbolt5"|"thunderbolt10"|"thunderbolt15"|"thunderbolt20"|"hyper_ac2"|"hyper_ac5"|"hyper_ac10"|"machine_gun"|"heavy_machine_gun"|"light_ac2"|"light_ac5"|"small_laser"|"medium_laser"|"large_laser"|"ppc"|"er_small_laser"|"er_medium_laser"|"er_large_laser"|"er_ppc"|"small_pulse_laser"|"medium_pulse_laser"|"large_pulse_laser"|"x_small_pulse_laser"|"x_medium_pulse_laser"|"x_large_pulse_laser"|"light_ppc"|"heavy_ppc"|"snub_nosed_ppc"|"srm2"|"rocket10"|"rocket15"|"rocket20"|"mrm10"|"mrm20"|"mrm30"|"mrm40"|"streak_srm2"|"streak_srm4"|"streak_srm6"|"lr_dfm5"|"lr_dfm10"|"lr_dfm15"|"lr_dfm20"|"sr_dfm2"|"sr_dfm4"|"sr_dfm6"|"elrm5"|"elrm10"|"elrm15"|"elrm20"|"lrm5"|"lrm10"|"lrm15"|"srm4"|"srm6"|"lrm20"|"heavy_gauss_rifle"|"gauss_rifle"|"light_gauss_rifle"|"magshot_gauss_rifle"|"lbx2"|"lbx5"|"lbx10"|"lbx20"|"ac2"|"ac5"|"ac10"|"ac20"|"ultra_ac2"|"ultra_ac5"|"ultra_ac10"|"ultra_ac20"|"rotary_ac2"|"rotary_ac5"|"clan_rotary_ac2"|"clan_rotary_ac5"|"clan_rotary_ac10"|"clan_rotary_ac20"

---@class BattleCriticalLocation
---@field section BattleSectionName
---@field slot integer Zero-based critical slot.

---@alias BattleAmmunitionMode "smoke"|"mine"|"i_narc_explosive"|"i_narc_haywire"|"i_narc_ecm"|"i_narc_nemesis"|"semi_guided"|"swarm"|"swarm1"|"stinger"|"narc"|"normal"|"cluster"|"artemis"|"precision"|"flechette"|"armor_piercing"|"caseless"|"incendiary"|"inferno"|"mml_lrm"|"extended_range"|"high_explosive"

---@alias BattleFireMode "normal"|"heat"|"hotload"|"ultra"|"rapid"|"rotary2"|"rotary4"|"rotary6"|"gatling"

---@class BattleWeaponMount
---@field weapon BattleWeapon
---@field criticals BattleCriticalLocation[] Slots belonging to one weapon.
---@field one_shot boolean Self-contained launcher; does not draw from ammunition bins.
---@field initially_spent boolean Initial template supply already expended.
---@field initial_ammunition_mode BattleAmmunitionMode
---@field initial_fire_mode BattleFireMode Template mode; live mode is reported by unit.weapons.
---@field rear_mount boolean
---@field on_targeting_computer boolean Explicit authored link, separate from automatic eligibility.
---@field brand integer|nil Manufacturer metadata.

---@class BattleAmmunitionBin
---@field location BattleCriticalLocation
---@field weapon BattleWeapon
---@field rounds integer Initial salvos in this independent bin.
---@field capacity integer Installed bin capacity in salvos.
---@field hotload boolean Retained bin flag; does not hotload the launcher.
---@field half_ton boolean Explicit half-ton construction flag.
---@field mode BattleAmmunitionMode
---@field brand integer|nil

---@class BattleSystemCritical
---@field location BattleCriticalLocation
---@field system string Snake_case system identity.
---@field brand integer|nil

---@class BattleLoadout
---@field weapons BattleWeaponMount[]
---@field ammunition BattleAmmunitionBin[]
---@field systems BattleSystemCritical[]

local btech_template = {}

---Read a biped asset without instantiating or activating a unit.
---@param name string Relative name under database.mech_database.
---@return BattleTemplate
function btech_template.inspect(name) end

---Resolve supported equipment; does not validate chassis construction or enable simulation.
---@param name string Relative name under database.mech_database.
---@return BattleLoadout
function btech_template.loadout(name) end

---@class BattleCargoTransferPoint
---@field x integer Zero-based map column.
---@field y integer Zero-based map row.
---@field reveal_hint boolean|nil Defaults to false; disclose coordinates in location failures only when true.

local btech_map = {}

---Read the detached saved cargo location, or nil when the map has no location restriction.
---@param map integer
---@return BattleCargoTransferPoint|nil
function btech_map.cargo_point(map) end

---Wizard-only transfer-point configuration; nil clears the point. Coordinates must lie inside the map.
---@param actor integer
---@param map integer
---@param point BattleCargoTransferPoint|nil
function btech_map.set_cargo_point(actor, map, point) end

---Read source-map metadata without decoding saved terrain or applying overlays.
---@param name string Relative name under database.map_database.
---@return BattleMapAssetSummary
function btech_map.inspect_file(name) end

---Inspect a saved map identity without activating simulation.
---@param dbref integer
---@return StoredBattleMap
function btech_map.inspect(dbref) end

---Read a decoded tile; ambiguous maps and invalid coordinates raise an error.
---@param dbref integer
---@param x integer Zero-based column.
---@param y integer Zero-based row.
---@return BattleHex
function btech_map.hex(dbref, x, y) end

---Register an existing room or thing using a source asset. Transactional.
---@param dbref integer
---@param name string Relative name under database.map_database.
---@return boolean
function btech_map.create(dbref, name) end

---Explicitly replace terrain on an unoccupied map of the same dimensions.
---@param dbref integer
---@param name string Relative name under database.map_database.
---@return boolean
function btech_map.reload(dbref, name) end

---@class BattleRadioChannel
---@field frequency integer Frequency from 0 through 999999.
---@field title string At most fifteen UTF-8 bytes.
---@field mode {digital: boolean, muted: boolean, relay: boolean, info: boolean, scan: boolean, color: string?}

local btech_unit = {}

---@class BattleVtolFuelStatus
---@field original_capacity integer Template fuel capacity.
---@field capacity integer Current capacity including 2000 per installed or carried auxiliary tank.
---@field remaining integer Saved fuel; -1 indicates announced exhaustion.
---@field auxiliary_tanks integer Carried Fuel_Tank items across manufacturers.
---@field installed_tanks integer Fuel_Tank criticals in saved VTOL construction.
---@field excess_mass integer Fuel above original capacity in 1/1024 tons before cargo discounts.

---Read detached VTOL fuel and current stock-derived capacity in a callback.
---@param unit integer
---@return BattleVtolFuelStatus
function btech_unit.fuel(unit) end

---Wizard fuel correction bounded by current capacity and 4294967295.
---Fuel and throttle changes participate in callback rollback; this does not repair lost lift.
---@param actor integer
---@param unit integer
---@param amount integer Nonnegative remaining fuel.
---@return BattleVtolFuelStatus
function btech_unit.set_fuel(actor, unit, amount) end
local btech_player = {}

---Inspect a saved unit identity without activating simulation.
---@param dbref integer
---@return StoredBattleUnit
function btech_unit.inspect(dbref) end

---@class BattleSectionState
---@field armor integer
---@field internal integer
---@field rear integer

---@class BattlePosition
---@field map integer Battlefield object dbref.
---@field x integer Zero-based column.
---@field y integer Zero-based row.

---@class BattlePower
---@field state "off"|"starting"|"running"
---@field remaining integer|nil Remaining committed seconds during startup.

---@class BattlePoint
---@field x number
---@field y number

---@class BattleMotion
---@field point table Continuous x/y measured in hex heights.
---@field heading number Current clockwise compass heading.
---@field desired_heading number
---@field speed number Current kph, negative for reverse.
---@field desired_speed number

---@class BattleMobility
---@field maximum_speed number Damage-adjusted maximum kph before terrain, heat and cargo.
---@field piloting_modifier integer Damage modifier for subsequent piloting checks.

---@alias BattleDetectionChannel "sensors"|"sight"|"radar"|"probe" Values from btech.unit.detection_channels.
---@alias BattlePerceptionStatus "ready"|"degraded"|"jammed"|"damaged"|"disabled"|"absent"

---@class BattleTargetLock
---@field target integer Selected unit dbref; may no longer be visible.
---@field remaining integer Settling seconds, 0..8; zero does not establish visibility.

---@class BattleStaggerHit
---@field damage integer
---@field remaining integer Seconds until this incoming damage group expires.
---@field counted boolean Whether a rolling check already used this group.

---@class BattleStagger
---@field action_damage integer Restored signed action-time scalar, independent of incoming damage history.
---@field hits BattleStaggerHit[]
---@field elapsed integer Committed seconds since the last rolling check.
---@field turn_damage integer Unchecked traditional damage.
---@field phase integer Saved per-unit turn phase, 0 through 29.
---@field checked_phase integer? Phase of the previous traditional check.

---@class BattleStandTimer
---@field state "rising"|"recovering"
---@field remaining integer Committed seconds remaining, 1..60.

---@class BattleMass
---@field engine integer Engine mass in 1/1024 tons.
---@field cockpit integer
---@field gyro integer Signed gyro accounting for destroyed center torsos.
---@field structure integer
---@field armor integer
---@field equipment integer
---@field ammunition integer
---@field cargo integer Authored cargo-space installation mass, excluding loose stock.
---@field total integer Current total in 1/1024 tons.

---@alias BattleLateralMode "none" | "front_left" | "front_right" | "rear_left" | "rear_right"

---@class BattleLateralState
---@field active BattleLateralMode
---@field pending BattleLateralMode?
---@field remaining integer Seconds until pending direction activates.

---@class BattleTransportState
---@field altitude number|nil Continuous altitude in terrain levels, including carried and pending-descent fractions.
---@field fortified boolean Scenario emplacement; blocks movement and towing, counts as immobile for aiming.
---@field towable boolean Explicit permission to tow this unit out of character.
---@field towing integer|nil Unit currently carried by this unit.
---@field towed_by integer|nil Carrier currently towing this unit.
---@field orbital_drop {elevation: integer, protection: {state: "cocoon"|"jump_jets", integrity: integer?}}|nil Saved orbital descent; inspection does not advance it.
---@field free_fall {elevation: number, speed: integer, remaining: integer, grounded: boolean}|nil Pending descent; inspection does not advance it.

---@class BattleHullDownState
---@field active boolean Completed lowered posture.
---@field pending boolean|nil Lowering (true) or raising (false).
---@field remaining integer Seconds left in the transition; zero when idle.

---@class BattleRadioState
---@field radio_experience_remaining integer Saved communication XP gate, 0 through 61 seconds.
---@field radio_skill integer Communication target captured on startup.
---@field radio BattleRadioChannel[] Active channels, indexed from one in this inspection array.
---@field radio_capabilities {channels: integer, range: integer, relay: boolean, digital: boolean, info: boolean, scan: boolean} Derived installed hardware limits.

---@class BattleUnitState: BattleTransportState, BattleRadioState
---@field mw_safety boolean MechWarrior safety; enabled when startup completes.
---@field bth_debug boolean Retained debug preference; combat reports do not consume this flag.
---@field last_startup integer Unix time of the last completed startup; zero before first completion.
---@field cockpit_links integer[] Three explicit cockpit destinations; unresolved references remain saved.
---@field preferred_id string? Configured two-letter preference; separate from the currently assigned ID.
---@field hull_down BattleHullDownState Quad hull-down posture and transition.
---@field kind "mech"
---@field mass BattleMass Derived current mass; detached from world state.
---@field searchlight_warning boolean Notify occupants on external illumination transitions.
---@field lateral BattleLateralState
---@field autocon_shutdown boolean Include shutdown targets in routine notices; defaults false.
---@field armor_warning boolean Armor threshold warnings; enabled by default.
---@field ammunition_warning boolean Low-ammunition warnings; enabled by default.
---@field friendly_fire_safety boolean Reject non-coolant fire at teammates; off by default.
---@field null_signature BattleSignatureState
---@field stealth BattleSignatureState
---@field electronics BattleElectronics Selected suite modes and last committed field.
---@field beacons table<BattleSection, BattleBeaconKind[]> Attached effects grouped by section.
---@field narc_sections BattleSection[] Sections carrying homing beacons.
---@field ams_enabled boolean Automatic anti-missile defense switch.
---@field auto_fall boolean Skip downhill cliff avoidance when piloted.
---@field hex_sync_pending boolean A collision interrupted synchronization of motion.point and position.
---@field elevation integer|nil Current signed altitude with terrain-effect jump rounding; nil when unplaced.
---@field stand_timer BattleStandTimer?
---@field reactor_instability_remaining integer? Damage window ticks remaining; nil uses initial world startup grace.
---@field triple_myomer_active boolean Derived from installed myomer and sampled excess heat.
---@field movement_maximum_speed number Current throttle ceiling, including active myomer.
---@field charge {target: integer?, elapsed: integer, distance: number} Persistent charge intent and movement counters.
---@field limb_recycle table<string, integer> Remaining physical recovery seconds by limb.
---@field stagger BattleStagger
---@field posture "standing"|"prone"
---@field flooded_sections string[] Persistent flooded section names.
---@field breached_sections string[] Persistent vacuum-disabled section names.
---@field map_slot integer|nil Persisted battlefield membership order.
---@field aimed_section BattleAimSelection|nil Saved anatomy preference; independent of the current lock.
---@field target_lock BattleTargetLock|BattleHexLock|nil
---@field sensor_ranges {tactical: integer, long_range: integer, scan: integer} Computer-derived hex limits after sensor damage.
---@field observer boolean Administrator-assigned observer role.
---@field combat_safe boolean Operator-imposed immunity to combat damage.
---@field weapons_hold boolean Operator-imposed firing restriction; mechanical readiness is independent.
---@field visibility {invisible: boolean, clairvoyant: boolean} Operator visibility state.
---@field battlefield_id string? Current battlefield identity; absent without map membership.
---@field searchlight {on: boolean, destroyed: boolean, remaining: integer} Hardware and pending five-second switch.
---@field fired_recently boolean Launched a weapon since the last heartbeat.
---@field spotter integer? Self ID while spotting, otherwise the selected observer.
---@field artillery_adjustment integer Saved correction for the current artillery target.
---@field spotter_events BattleSpotterEvents Pending radio requests and periodic checks.
---@field tag BattleTagState
---@field signature {team: integer, hidden: boolean, illuminated: boolean} Team, hiding and scenario lighting.
---@field scanner_perception integer Perception captured at startup completion.
---@field facing {torso: "left"|"center"|"right"|"both", arms_flipped: boolean}
---@field stun_remaining integer Remaining seconds of cockpit stun.
---@field pilot_injuries integer Tactical injury count; six means scenario pilot loss.
---@field self_destruct {remaining: integer, ammunition: boolean}|nil Admitted timer and its actual Mech detonation mode.
---@field self_destruct_safe boolean Scenario protection from new ammunition self-destruct requests.
---@field hide_elapsed integer|nil Elapsed camouflage checks; nil when no hide event is pending.
---@field crew_recovery_remaining integer Empty-crew consciousness countdown; random state stays private.
---@field character_pilot {injuries: integer, killed: boolean}? Saved character-mode injury status; character health determines death.
---@field heat_cutoff {enabled: boolean, disabled: integer, remaining: integer|nil} Intentional cooling suppression and seconds until the toggle completes.
---@field last_jump {heading: integer, length: integer} Current course bearing and signed length in field units, retained after landing.
---@field heat_sample {production: number, dissipation: number} Last committed thermal sample; production includes stored weapon heat.
---@field heat {stored: number, excess: number} Weapon heat (possibly negative coolant credit until the next sample) and sampled excess heat.
---@field inferno_remaining integer Saved burn seconds; cooling is reduced by six while positive.
---@field overheat_clock {elapsed: integer, phase: integer, injury_due: boolean} Saved committed-second thermal checks.
---@field weapon_recycle table<integer, integer> Remaining seconds keyed by zero-based weapon index.
---@field component_failures {location: table, failure: string}[] Nonweapon diagnostic conditions; material damage determines system operation.
---@field weapon_failures table<integer, "jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam"> Temporary conditions by mount index; existing recycle clocks govern recovery.
---@field gyro "standard"|"hardened"|"xl"|"compact" Construction family.
---@field artemis BattleArtemisController[] Installed controllers and resolved links.
---@field weapon_damage BattleWeaponDamage[]|nil Mech weapon critical degradation.
---@field masc BattleBoosterState Saved activation and overload/recovery state.
---@field supercharger BattleBoosterState Independent compressor timer and failure state.
---@field supercharger_installed boolean Template technology flag.
---@field supercharger_operational boolean Technology remains available and has not failed.
---@field c3_members integer[] Classic C3 members retained by current working-master capacity.
---@field c3_operational boolean Working classic C3 hardware.
---@field c3i_members integer[] Eligible network members including this unit, empty when disconnected; shutdown and ECM retain membership.
---@field c3_hardware {masters: integer, working_masters: integer, slave_installed: boolean, slave_operational: boolean, c3i_installed: boolean, c3i_operational: boolean} Installed and working command-network computers; independent of power and membership.
---@field masc_installed boolean Sufficient MASC hardware is installed.
---@field masc_operational boolean Enough MASC slots remain functional; does not indicate activation.
---@field unjam BattleUnjam|nil Active feed recovery.
---@field dumping table|nil Active ammunition selection and elapsed cadence.
---@field gyro_damage integer Effective gyro damage after hardened protection.
---@field mobility BattleMobility
---@field jump_capacity {speed: number, movement_points: integer} Damage/gravity-adjusted capacity; does not authorize flight. Unplaced units use 100% gravity.
---@field flight {path: {start: BattlePoint, end: BattlePoint, start_elevation: number, end_elevation: integer, movement_points: integer, continuation: boolean, projection: {bearing: integer, range: number}|nil, target_range: number|nil}, travelled: number, completed_distance: number, landing_requested: boolean, sampled_movement_points: integer, dfa_target: integer|nil}|nil
---@field airborne {point: BattlePoint, elevation: number}|nil Last committed airborne sample.
---@field jump_stabilization integer Remaining seconds, zero through twelve.
---@field engine "standard"|"light"|"xl"|"xxl"|"compact" Installed fusion-engine family.
---@field destroyed boolean Core structure, cockpit or engine is destroyed.
---@field lost_criticals BattleCriticalLocation[] Explicit destroyed equipment slots.
---@field motion BattleMotion|nil
---@field power BattlePower
---@field pilot integer|nil Player in the cockpit; must be physically inside this unit.
---@field position BattlePosition|nil
---@field definition BattleTemplate Owned definition, independent of source files.
---@field sections table<BattleSectionName, BattleSectionState>
---@field ammunition integer[] Remaining salvos in resolved bin order.

---@class BattleVehicleMass
---@field engine integer
---@field cockpit integer
---@field components integer
---@field turret integer
---@field structure integer
---@field armor integer
---@field equipment integer
---@field cooling integer
---@field cargo integer
---@field ammunition integer Loaded ammunition mass.
---@field ammunition_capacity integer Full surviving bin mass.
---@field total integer Current physical mass in 1/1024-ton units.
---@field design_total integer Current component total with full surviving bins.

---@class BattleDigState
---@field dug_in boolean Whether cover applies.
---@field digging boolean Whether preparation is active.
---@field completion integer[] Pending completion deadlines in seconds; empty means none.

---@class BattleVehicleState: BattleTransportState, BattleRadioState
---@field armor_warning boolean Armor severity warnings; enabled by default.
---@field ammunition_warning boolean Low-ammunition warnings; enabled by default.
---@field searchlight {on: boolean, destroyed: boolean, remaining: integer} Hardware and pending five-second switch.
---@field autocon_shutdown boolean Include shutdown targets in routine contact notices.
---@field searchlight_warning boolean Announce external illumination transitions.
---@field mw_safety boolean MechWarrior safety; enabled when startup completes.
---@field bth_debug boolean Retained debug preference; combat reports do not consume this flag.
---@field last_startup integer Unix time of the last completed startup; zero before first completion.
---@field cockpit_links integer[] Three explicit cockpit destinations; unresolved references remain saved.
---@field preferred_id string? Configured two-letter preference; separate from the currently assigned ID.
---@field fuel BattleVtolFuelStatus|nil Live fuel projection for VTOLs only.
---@field fired_recently boolean A weapon launched since the last heartbeat.
---@field observer boolean Administrator-assigned observer role.
---@field combat_safe boolean Operator-imposed immunity to combat damage.
---@field weapons_hold boolean Operator-imposed firing restriction; mechanical readiness is independent.
---@field visibility {invisible: boolean, clairvoyant: boolean} Operator visibility state.
---@field dig BattleDigState Saved ground-vehicle cover preparation.
---@field mass BattleVehicleMass Derived from current material and ammunition; units are 1/1024 ton.
---@field inferno_remaining integer Stationary-unit jelly duration.
---@field burning_sections table<string, integer> Section fire countdowns in seconds.
---@field extinguishing integer|nil Seconds until the crew completes its attempt.
---@field pod_removal integer|nil Remaining seconds of the crew iNarc-removal attempt.
---@field beacons table<string, string[]> Attached effects keyed by surviving vehicle section.
---@field ams_enabled boolean Saved automatic anti-missile defense switch.
---@field artemis BattleArtemisController[] Installed controllers and resolved links.
---@field unjam BattleUnjam|nil Active feed clearing attempt.
---@field weapon_recycle table<integer, integer> Countdown seconds by zero-based weapon index.
---@field spent_launchers integer[] Expended zero-based one-shot weapon indices.
---@field lost_criticals table[] Destroyed vehicle equipment locations, each with section and zero-based slot.
---@field piloting_damage integer Cumulative vehicle handling penalty.
---@field component_failures {location: table, failure: string}[] Nonweapon diagnostic conditions; material damage determines system operation.
---@field weapon_failures table<integer, "jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam"> Temporary conditions by mount index; existing recycle clocks govern recovery.
---@field crew_stun_remaining integer Seconds until the pending recovery event; zero means none.
---@field crew_stunned boolean Effective crew stun, independent of its timer.
---@field self_destruct {remaining: integer, ammunition: boolean}|nil Admitted timer and its actual Mech detonation mode.
---@field self_destruct_safe boolean Scenario protection from new ammunition self-destruct requests.
---@field hide_elapsed integer|nil Elapsed camouflage checks; nil when no hide event is pending.
---@field crew_recovery_remaining integer Empty-crew consciousness countdown, separate from crew stun.
---@field weapon_heat number Passive weapon heat and coolant credit; ground vehicles do not overheat.
---@field gunnery_damage integer Cumulative firing penalty from sensor and commander damage.
---@field lost_stabilizers string[] Sections with destroyed weapon stabilizers.
---@field signature {team: integer, hidden: boolean, illuminated: boolean} Team, hiding and scenario lighting.
---@field scanner_perception integer Perception captured at startup completion.
---@field sensor_ranges {tactical: integer, long_range: integer, scan: integer} Computer-derived hex limits.
---@field aimed_section BattleAimSelection|nil Saved anatomy preference; independent of the current lock.
---@field target_lock BattleTargetLock|BattleHexLock|nil Saved selection and settling countdown.
---@field artillery_adjustment integer Saved correction for the selected artillery coordinate.
---@field c3_hardware {masters: integer, working_masters: integer, slave_installed: boolean, slave_operational: boolean, c3i_installed: boolean, c3i_operational: boolean} Installed and working command-network computers; independent of power and membership.
---@field c3i_members integer[] Eligible members in the improved command network.
---@field c3_members integer[] Eligible members in the classic command network.
---@field flooded boolean Permanently disabled by water, independently of armor and crew health.
---@field breached_sections string[] Persisted vacuum breaches; equipment is disabled without destroying slots or expending ammunition.
---@field crew_killed boolean Instant crew loss, independent of tactical and character injury counts.
---@field electronics BattleElectronics Selected suite modes and last committed field.
---@field spotter integer|nil Self declares spotting; another unit selects a forward observer.
---@field spotter_events BattleSpotterEvents Pending radio requests and periodic checks.
---@field tag BattleTagState Shared TAG selection and lock/recycle countdown.
---@field character_pilot {injuries: integer, killed: boolean}? Saved RPG pilot status, independent of tactical injury count.
---@field friendly_fire_safety boolean Pilot-selected teammate protection.
---@field auto_fall boolean Skip downhill cliff avoidance when piloted.
---@field brief BattleBriefSettings
---@field fire_modes table<integer, BattleFireMode> Selected non-normal firing modes by zero-based weapon index.
---@field ammunition_modes table<integer, BattleAmmunitionMode> Selected non-normal modes by zero-based weapon index.
---@field turret_heading number|nil Absolute heading of a surviving turret.
---@field automatic_turret boolean Pilot-selected automatic unit/hex target tracking.
---@field turret_jammed boolean Recoverable turret rotation damage.
---@field turret_repairs integer[] Pending 60-second repair attempts.
---@field turret_locked boolean Turret damage prevents rotation.
---@field maximum_speed number Current maximum kph after motive damage.
---@field motive_speed_loss number Maximum speed lost to motive damage in kph.
---@field immobilized boolean Motive-system destruction prevents ground motion.
---@field under_bridge boolean Hovercraft beneath a bridge span.
---@field elevation integer|nil Ground support height; hovercraft float at water level.
---@field motion BattleMotion|nil
---@field pilot integer|nil Assigned cockpit operator.
---@field power BattlePower
---@field kind "vehicle"
---@field simulation_supported false Full vehicle terrain and combat support is unfinished.
---@field definition table Owned ground-vehicle definition.
---@field sections table<string, BattleSectionState> Vehicle faces: left, right, front, rear, turret.
---@field ammunition integer[] Remaining rounds in resolved bin order.
---@field position BattlePosition|nil
---@field map_slot integer|nil
---@field destroyed boolean Any hull face has lost its internal structure.

---Construct a persistent Mech or ground vehicle on an unused live thing. Transactional.
---@param dbref integer
---@param name string Relative name under database.mech_database.
---@return boolean
function btech_unit.create(dbref, name) end

---Inspect detached construction state; deferred saved units raise an error.
---@param dbref integer
---@return BattleUnitState|BattleVehicleState
function btech_unit.state(dbref) end

---Place a unit on decoded terrain and update world containment. Transactional.
---@param dbref integer
---@param map integer
---@param x integer
---@param y integer
---@return boolean
function btech_unit.place(dbref, map, x, y) end

---Clear placement and move a unit into an ordinary container. Transactional.
---@param dbref integer
---@param destination integer
---@return boolean
function btech_unit.remove(dbref, destination) end

---Assign a present player to an available cockpit. The caller supplies access policy.
---@param dbref integer Unit object.
---@param player integer Pilot object.
---@return boolean
function btech_unit.pilot(dbref, player) end

---Wizard-only team edit for a placed unit; negatives normalize to zero, other signature facts are retained.
---@param actor integer
---@param unit integer
---@param team integer Signed 32-bit team number.
---@return integer team Normalized value, also reported privately to the administrator.
function btech_unit.set_team(actor, unit, team) end

---Wizard-only literal emote to running units currently seeing the source; source cockpit excluded.
---Privately confirms completion. Entire publication rolls back on failure.
---@param actor integer
---@param unit integer Placed physical unit; need not be running or piloted by actor.
---@param message string Empty text and leading apostrophes retain ordinary emote semantics.
---@return integer observers Number of addressed observer units.
function btech_unit.losemit(actor, unit, message) end

---Wizard-only saved ID preference; does not change the current label or consume dice.
---Rust extension retained under its descriptive name; the canonical setter follows the C contract.
---@param actor integer Wizard actor.
---@param unit integer Constructed unit; no placement or power requirement.
---@param value? string Exactly two ASCII letters; nil or empty clears the preference.
---@return string? preferred_id Normalized uppercase preference, or nil when cleared.
function btech_unit.set_preferred_id_as(actor, unit, value) end

---Set the saved two-letter battlefield ID preference; nil clears it without consuming dice.
---@param unit DbRef|Object Constructed unit.
---@param id string|nil Exactly two ASCII letters; nil clears the preference.
function btech_unit.set_preferred_id(unit, id) end

---Read the saved display-name override, or nil when the template name is in use.
---@param unit DbRef|Object
---@return string|nil name
function btech_unit.display_name(unit) end

---Set a display override of at most 120 bytes; an empty string clears it. Wizard only.
---Rust extension retained under its descriptive name; the canonical setter follows the C contract.
---@param actor integer
---@param unit integer
---@param name string
---@return boolean success
function btech_unit.set_display_name_as(actor, unit, name) end

---Replace the saved display-name override; nil or an empty string clears it.
---@param unit DbRef|Object Constructed unit.
---@param name string|nil At most 120 bytes.
function btech_unit.set_display_name(unit, name) end

---Read the saved markings string, or nil when no markings are configured.
---@param unit DbRef|Object
---@return string|nil markings
function btech_unit.markings(unit) end

---Wizard-only literal markings, at most 16383 bytes; empty clears. Callback failures roll back.
---Rust extension retained under its descriptive name; the canonical setter follows the C contract.
---@param actor integer
---@param unit integer
---@param markings string
---@return boolean success
function btech_unit.set_markings_as(actor, unit, markings) end

---Replace the saved markings, at most 16383 bytes; nil or an empty string clears them.
---@param unit DbRef|Object Constructed unit.
---@param markings string|nil
function btech_unit.set_markings(unit, markings) end

---View escaped markings through running cockpit contact and unblocked-LOS admission.
---No scan-range limit; omitted target uses this operator's selected unit.
---@param unit integer Cockpit unit.
---@param actor integer
---@param target? integer
---@return string text Styled report safe for normal output.
function btech_unit.view(unit, actor, target) end

---Wizard map assignment; -1 removes membership and retains the pose for re-entry.
---A removed running unit shuts down on the next simulation update.
---@param actor integer Wizard and private confirmation recipient.
---@param unit integer Physical constructed unit.
---@param map integer Decimal map dbref, or -1 for removal.
---@param preferred? string First two bytes override configuration; short/nil values use the saved preference, then random selection.
---@return {assignment: table|nil} report Assigned position, label and reset_origin, or nil on removal.
function btech_unit.setmapindex(actor, unit, map, preferred) end

---Wizard repositioning within the current battlefield, preserving controls and tow attachment.
---@param actor integer Wizard actor and confirmation recipient.
---@param unit integer Placed physical unit.
---@param x integer
---@param y integer
---@param z? integer Signed-short altitude; nil selects the surface and lands VTOLs.
---@return {position: table, elevation: integer} report
function btech_unit.setxy(actor, unit, x, y, z) end

---Wizard orbital insertion. Detach towing first; reject prone units and active digging.
---Ground chassis receive mass-based cocoons. VTOLs enter flight with half-speed requested.
---Stopped VTOLs retain the inserted pose and controls until startup finishes.
---@param actor integer Wizard actor and confirmation recipient.
---@param unit integer Placed physical unit.
---@param x integer
---@param y integer
---@param z? integer Signed-short altitude; nil defaults to 300.
---@return {position: table, elevation: integer, drop: table|nil, flight: table|nil} report
---An attached on_ood_land event runs before landing dice/damage, with the arriving unit as object/enactor/cause.
---Callback errors restore the entire airborne tick and its output.
function btech_unit.ood(actor, unit, x, y, z) end

---Wizard-only construction allocation using original components and installed ammunition bins.
---@param actor integer Recipient; requires wizard authority.
---@param unit integer Power, pilot assignment and placement are not required.
---@return string report Publishes privately and returns formatted weight rows and total.
function btech_unit.weight(actor, unit) end

---Wizard-only random damage packets through shared combat and casualty rules.
---@param actor integer
---@param unit integer Power, pilot and placement are not required.
---@param damage integer 1 through 1000.
---@param clusters integer Packet count from 1 through damage; integer division discards the remainder.
---@param rear boolean Select rear armor, also enabled by a rear incoming arc.
---@param critical boolean Accepted flag; random location routing chooses critical eligibility.
---@return {packet_damage: integer, discarded_damage: integer, impacts: table[]} report
function btech_unit.damage(actor, unit, damage, clusters, rear, critical) end

---Wizard-only located damage through shared critical, crew and evacuation rules.
---@param actor integer
---@param unit integer
---@param section string Chassis-specific location or abbreviation.
---@param damage integer 1 through 1000.
---@param rear boolean Rear armor selection; vehicle front hits redirect to rear.
---@param critical boolean Through-armor critical candidate.
---@return {kind: "mech"|"vehicle", impact: table} report
function btech_unit.damage_section(actor, unit, section, damage, rear, critical) end

---Release this player's cockpit assignment without moving the player.
---@param dbref integer
---@param player integer
---@return boolean
function btech_unit.release(dbref, player) end

---Start the assigned pilot's unit. Trusted scripts authorize the optional fast override.
---@param dbref integer
---@param player integer
---@param fast boolean|nil Five-second override; otherwise 30 seconds.
---@return boolean
function btech_unit.start(dbref, player, fast) end

---Abort startup or shut down the assigned pilot's unit, releasing the cockpit.
---@param dbref integer
---@param player integer
---@return boolean
function btech_unit.stop(dbref, player) end

---@class BattleRange
---@field horizontal number Horizontal Euclidean range in hex heights.
---@field spatial number Euclidean range including signed ground elevation/depth.
---@field bearing number|nil Degrees clockwise from north; nil for coincident centers.
---@field hex_distance integer Minimum adjacent hex steps, without terrain costs.

---Measure placed units on the same map. Does not check visibility or weapon eligibility.
---@param first integer
---@param second integer
---@return BattleRange
function btech_unit.range(first, second) end

---Set desired heading on the current pilot's running unit and stage a cockpit confirmation.
---@param dbref integer
---@param player integer
---@param degrees number? Omit to read actual heading without notification.
---@return boolean|number
function btech_unit.heading(dbref, player, degrees) end

---Set desired speed within the running unit's forward/reverse limits and stage a cockpit confirmation.
---@param dbref integer
---@param player integer
---@param kph number|string? Omit to read actual speed; names include walk/cruise, run/flank, stop and back. Numeric requests clamp to throttle limits.
---@return boolean|number
function btech_unit.speed(dbref, player, kph) end

---Evacuate non-wizard contents of an in-character unit to the configured afterlife.
---Requires a wizard actor and callback. Ordinary teleport hooks run; failures roll back moves and XP.
---Tactical units do nothing. Configured XP retention applies only when in-character rules are enabled.
---@param dbref integer Unit object.
---@param player integer Wizard actor.
---@return integer Number of occupants moved.
function btech_unit.evacuate(dbref, player) end

---@class BattleCharacter
---@field perception_target integer Target derived from intuition, learning and effective Perception skill.
---@field values table<string, {value: integer, experience: integer, last_used: integer}> Detached named skill/advantage records.
---@field unconscious_remaining integer Seconds before the next recovery attempt; zero when conscious.
---@field bruise integer
---@field lethal integer
---@field build integer
---@field reflexes integer
---@field intuition integer
---@field learn integer
---@field charisma integer
local btech_character = {}

---@class BattleAdvantageDefinition
---@field name string Canonical advantage name.
---@field kind "boolean"|"ranked"|"attribute_mask" Boolean values activate only at one.

---Return the detached catalog of supported advantages; gameplay availability varies by action.
---@return BattleAdvantageDefinition[]
function btech_character.advantages() end

---@class BattleSkillDefinition
---@field name string Canonical storage name.
---@field category "athletic"|"mental"|"physical"|"social"
---@field threshold integer Default experience threshold.
---@field continuous boolean Whether awards bypass the thirty-second interval.

---Return a detached catalog in canonical lookup order.
---@return BattleSkillDefinition[]
function btech_character.skills() end

---List canonical names in catalog order. An optional player filters skills with nonzero value or XP.
---Advantages and attributes remain complete when a player is supplied. Requires a callback.
---@param kind "skills"|"advantages"|"attributes" Full category names are case-insensitive; abbreviations are rejected.
---@param player? integer|string Live player id, name, account alias or #dbref; omit rather than passing explicit nil.
---@return string[]
function btech_character.list(kind, player) end

---@class BattleSkillProgress
---@field name string Canonical skill name.
---@field target integer Current skill target including stored earned levels.
---@field raw_target integer Skill target excluding earned levels.
---@field earned_levels integer Stored XP bonus.
---@field balance integer Low 24-bit experience balance.
---@field threshold integer Current runtime threshold.
---@field next_level_balance integer? Total balance needed for the next stored level; nil when disabled.
---@field remaining integer? Additional points needed; zero if recalculation is overdue.

---Inspect progress without changing XP. Requires a callback and existing character attributes.
---@param player integer
---@param skill string Canonical name or short alias.
---@return BattleSkillProgress
function btech_character.progress(player, skill) end

---Current runtime XP threshold; fails for unknown skills. Requires a callback.
---@param skill string Canonical name or short alias.
---@return integer
function btech_character.threshold(skill) end

---Set a runtime XP threshold as a wizard. Defaults return after database reload.
---Existing earned levels are recalculated on the next accepted award.
---@param player integer Wizard actor.
---@param skill string Canonical name or short alias.
---@param threshold integer From 0 through 2147483647; zero disables earned levels.
---@return boolean
function btech_character.set_threshold(player, skill, threshold) end

---Detached saved character attributes and health; fails when no profile exists.
---@param player integer
---@return BattleCharacter
function btech_character.state(player) end

---Change saved light/weather conditions without reloading occupied terrain.
---Perception follows the new light and visibility on the next scan; contacts and locks remain until then.
---@param dbref integer Map object dbref.
---@param light BattleLightLevel Typed constant from btech.map.light_levels.
---@param visibility integer Weather range from 0 through 60.
---@return boolean
function btech_map.conditions(dbref, light, visibility) end

---Wizard-only persisted cloud boundary. Zero disables it; accepts signed 16-bit elevation levels.
---@param actor integer
---@param dbref integer
---@param altitude integer
---@return integer altitude
function btech_map.cloud_base(actor, dbref, altitude) end

---@class BattleHexCoordinate
---@field x integer
---@field y integer

---@class BattleSurfaceBreak
---@field map integer
---@field coordinate BattleHexCoordinate
---@field before BattleHex
---@field after BattleHex
---@field fall_levels integer
---@field falls table[] Ordered pairs of unit dbref and Mech fall report.
---@field vehicle_falls table[] Ordered pairs of unit dbref and vehicle fall report.
---@field flooded_vehicles integer[]
---@field notices table[] Unit dbrefs and cockpit message text.

---Wizard broadcast to occupants of running, conscious units in map-slot order.
---Does not require sensor contacts; all notices and the private confirmation roll back together.
---Rust extension retained under its descriptive name; the canonical emit follows the C contract.
---@param actor integer
---@param dbref integer
---@param text string Leading spaces are removed; empty messages are rejected.
---@return integer[] Eligible unit dbrefs, including units with empty cockpits.
function btech_map.emit_as(actor, dbref, text) end

---@class BattleMapEmitOptions
---@field audience? "all"|"range"|"line_of_sight" Recipient selection; defaults to all.
---@field origin? BattleHexCoordinate Required anchor for range and line_of_sight audiences.
---@field range? number Nonnegative hex radius; required with the range audience.

---Deliver a cockpit message to occupants of running units using the shared transactional emitter.
---@param map DbRef|Object
---@param message string One through 8191 bytes; leading spaces are removed.
---@param options? BattleMapEmitOptions
function btech_map.emit(map, message, options) end

---Wizard-only shutdown and removal in map-slot order; game objects stay in the map room.
---@param actor integer
---@param dbref integer
---@return integer[] Removed unit dbrefs.
function btech_map.clear_units(actor, dbref) end

---Wizard-only resize; copies overlapping visible tiles, clears map objects and rejects clipped units.
---@param actor integer
---@param dbref integer
---@param width integer 1 through 1000
---@param height integer 1 through 1000
---@return boolean
function btech_map.resize(actor, dbref, width, height) end

---Stage an atomic asset replacement after world commit; true means queued, not written.
---@param actor integer Wizard receiving the completion or failure notice.
---@param dbref integer
---@param name string Relative name inside the configured map directory.
---@return boolean
function btech_map.save(actor, dbref, name) end

---Load an asset; GOD keeps membership, while other wizards shut down and clear units.
---Rust extension retained under its descriptive name; the canonical loader follows the C contract.
---@param actor integer
---@param dbref integer
---@param name string Relative asset name.
---@return boolean
function btech_map.load_as(actor, dbref, name) end

---Replace map terrain from a saved asset using the strict C contract.
---@param map DbRef|Object
---@param name string Relative name under database.map_database.
function btech_map.load(map, name) end

---Install wizard fire; zero duration is permanent. Off-map coordinates leave the map unchanged.
---@param actor integer
---@param dbref integer
---@param x integer
---@param y integer
---@param duration integer Signed seconds; fire keeps a signed-short spread budget, smoke uses at least one tick.
---@return boolean
function btech_map.add_fire(actor, dbref, x, y, duration) end

---Add a newest-first mine owned by the wizard; return its persistent record slot.
---@param actor integer
---@param map integer
---@param x integer
---@param y integer
---@param kind 'standard'|'inferno'|'command'|'vibra'|'trigger'
---@param strength integer
---@param extra? integer
---@return integer
function btech_map.add_mine(actor, map, x, y, kind, strength, extra) end

---Publish a labelled terrain-only map using the wizard's saved display preferences.
---@param actor integer
---@param map integer
---@param x integer
---@param y integer
---@return table report Clipped viewport and styled text; maximum_range is zero without a scanner.
function btech_map.view(actor, map, x, y) end

---Edit an exact named map field as a wizard; shared controls publish any cockpit consequences.
---@param actor integer
---@param map integer
---@param field string
---@param value string
function btech_map.set_field(actor, map, field, value) end

---Install wizard smoke; zero duration is permanent. Off-map coordinates leave the map unchanged.
---@param actor integer
---@param dbref integer
---@param x integer
---@param y integer
---@param duration integer Signed seconds; fire keeps a signed-short spread budget, smoke uses at least one tick.
---@return boolean
function btech_map.add_smoke(actor, dbref, x, y, duration) end

---@class BattleAuthoredMapLink
---@field parent integer Parent map.
---@field coordinate BattleHexCoordinate Placement on the parent.
---@field entrances? table[] Four cardinal modes, north/east/south/west: {kind="none"}, {kind="offset",distance=N}, or {kind="exact",coordinate={x=X,y=Y}}.

---Read the authored link configuration saved by the wizard editor. Rust extension retained
---under its descriptive name; the canonical link follows the C contract.
---@param child integer
---@return BattleAuthoredMapLink|nil
function btech_map.authored_link(child) end

---Configure an authored link without rebuilding live routes; nil removes the configuration.
---Rust extension retained under its descriptive name; the canonical setter follows the C contract.
---@param child integer
---@param link BattleAuthoredMapLink|nil
---@return boolean
function btech_map.set_authored_link(child, link) end

---@alias BattleMapEntrance {mode: "offset", offset: integer}|{mode: "exact", x: integer, y: integer}

---@class BattleMapEntrances
---@field north? BattleMapEntrance
---@field east? BattleMapEntrance
---@field south? BattleMapEntrance
---@field west? BattleMapEntrance

---@class BattleMapLink
---@field parent Object Parent map object.
---@field x integer Placement column on the parent.
---@field y integer Placement row on the parent.
---@field entrances? BattleMapEntrances

---Read the strict C-contract link configuration of a child map, or nil when none is authored.
---@param child DbRef|Object
---@return BattleMapLink|nil
function btech_map.link(child) end

---Replace the C-contract link configuration of a child map; nil removes it.
---@param child DbRef|Object
---@param link BattleMapLink|nil
function btech_map.set_link(child, link) end

---Rebuild reachable map routes with cycle/depth protection and atomic publication.
---Rust extension retained under its descriptive name; the canonical rebuild follows the C contract.
---@param actor integer
---@param map integer
---@return {buildings:integer,leaves:integer,entrances:integer,skipped:integer}
function btech_map.update_links_as(actor, map) end

---Rebuild reachable map routes with cycle/depth protection and atomic publication.
---@param map DbRef|Object
function btech_map.update_links(map) end

---Publish a wizard map listing without advancing simulation or changing contacts.
---@param actor integer
---@param dbref integer
---@param target string MECHS or OBJS; complete case-insensitive name required.
---@return boolean
function btech_map.list(actor, dbref, target) end

---Delete map objects by type, coordinate, or both. At least one selector is required.
---@param actor integer
---@param dbref integer
---@param kind? string FIRE, SMOKE, DECO, MINE, BUILDING, LEAVE, ENTRA, LINKED, or BLZ; prefixes accepted.
---@param x? integer Must be paired with y.
---@param y? integer Must be paired with x.
---@return integer Number of selected records deleted; reciprocal cleanup is not counted.
function btech_map.delete_objects(actor, dbref, kind, x, y) end

---Add a wizard-owned circular landing restriction; negative radii block nothing.
---@param actor integer
---@param dbref integer
---@param x integer
---@param y integer
---@param radius integer
---@param team? integer Zero means no exemption.
---@return integer Restriction slot.
function btech_map.add_block(actor, dbref, x, y, radius, team) end

---@class BattleMapHexChange
---@field map integer
---@field coordinate BattleHexCoordinate
---@field before BattleHex
---@field after BattleHex

---Wizard live base-terrain edit. Retains unit altitude and overlays; does not cause combat falls.
---@param actor integer
---@param dbref integer
---@param x integer
---@param y integer
---@param terrain string Canonical terrain symbol; a leading dot selects grassland.
---@param elevation integer Absolute magnitude capped at nine.
---@return BattleMapHexChange
function btech_map.set_hex(actor, dbref, x, y, terrain, elevation) end

---@class BattleMapIceReport
---@field map integer
---@field changed BattleHexCoordinate[] Coordinates in column-major processing order.
---@field fractures BattleSurfaceBreak[] Melting consequences, including affected occupants.

---Wizard seasonal growth. Only water can freeze; new ice does not extend this pass's shoreline.
---@param actor integer
---@param dbref integer
---@param percentage integer Signed percentage threshold; outside 0–100 means never/always.
---@return BattleMapIceReport
function btech_map.add_ice(actor, dbref, percentage) end

---Wizard seasonal melting. Falls, flooding, casualties and notices commit with the terrain.
---@param actor integer
---@param dbref integer
---@param percentage integer
---@return BattleMapIceReport
function btech_map.remove_ice(actor, dbref, percentage) end

---@class BattleMapEnvironment
---@field gravity integer Percent of Earth gravity, 0 through 255.
---@field temperature integer Celsius, -128 through 127.
---@field vacuum boolean? Defaults to false, clearing existing vacuum.
---@field underground boolean? Defaults to false; existing underground status is retained.

---Wizard SETCOND action; updates live map rules without advancing time or resetting units.
---@param actor integer
---@param dbref integer
---@param conditions BattleMapEnvironment
---@return BattleMapEnvironment Actual resulting state, including retained underground status.
function btech_map.environment(actor, dbref, conditions) end

---Enable or disable saved opposite-edge wrapping.
---@param dbref integer
---@param enabled boolean
---@return boolean
function btech_map.wrapping(dbref, enabled) end

---@class BattleBlastZone
---@field x integer
---@field y integer
---@field radius integer

---List saved artillery blast zones in saved order.
---@param map DbRef|Object
---@return BattleBlastZone[]
function btech_map.blast_zones(map) end

---Read the saved cargo transfer point, or nil when the map has no location restriction.
---@param map DbRef|Object
---@return BattleCargoTransferPoint|nil
function btech_map.cargo_transfer_point(map) end

---Replace the saved cargo transfer point; nil clears the restriction.
---@param map DbRef|Object
---@param point BattleCargoTransferPoint|nil
function btech_map.set_cargo_transfer_point(map, point) end

---Read one tile elevation; water and ice report depth.
---@param map DbRef|Object
---@param hex BattleHexCoordinate
---@return integer elevation
function btech_map.elevation(map, hex) end

---@alias BattleTerrainName "grassland"|"road"|"light_forest"|"heavy_forest"|"water"|"ice"|"bridge"|"high_water"|"rough"|"mountains"|"fire"|"smoke"|"snow"|"building"|"wall"

---Read one decoded terrain kind.
---@param map DbRef|Object
---@param hex BattleHexCoordinate
---@return BattleTerrainName terrain
function btech_map.terrain(map, hex) end

---Report whether a coordinate lies inside a saved blast zone.
---@param map DbRef|Object
---@param hex BattleHexCoordinate
---@return boolean inside
function btech_map.in_blast_zone(map, hex) end

---@alias BattleLineOfSight "none"|"blocked"|"clear"

---Report line of sight from one placed unit toward a unit or hex.
---@param observer DbRef|Object
---@param target DbRef|Object|BattleHexCoordinate
---@return BattleLineOfSight state
function btech_map.line_of_sight(observer, target) end

---@alias BattlePlacement {x: integer, y: integer, z?: integer}

---Place a unit on decoded terrain using the shared placement rules.
---@param unit DbRef|Object
---@param map DbRef|Object
---@param position BattlePlacement
function btech_map.place_unit(unit, map, position) end

---Measure the spatial range between two units or positions on one map.
---@param map DbRef|Object
---@param from DbRef|Object|BattlePlacement
---@param to DbRef|Object|BattlePlacement
---@return number range
function btech_map.range(map, from, to) end

---Resolve the first unit matching a two-character battlefield ID from a unit or map origin.
---@param origin DbRef|Object Registered unit or map.
---@param id string Exactly two ASCII characters.
---@return Object|nil unit
function btech_map.unit_by_id(origin, id) end

---@class BattleMapUnitFilter
---@field origin BattleHexCoordinate Filter anchor.
---@field range number Nonnegative hex radius.

---List units placed on a map in saved slot order; an optional filter omits distant units.
---@param map DbRef|Object
---@param filter? BattleMapUnitFilter
---@return Object[] units
function btech_map.units(map, filter) end

---Read-only summary of a placed unit's automatic perception: sensor band, sight, probe and radar.
---@param dbref integer
---@return BattlePerceptionReport
function btech_unit.perception(dbref) end

---@class BattlePerceptionReport
---@field light "night"|"twilight"|"day" Current battlefield light.
---@field sight_range integer Weather visibility in hexes, capped by the map ceiling.
---@field lit_sight_range integer Reach to illuminated targets; triple sight at night.
---@field sensor_range integer Effective all-conditions sensor band; zero while unavailable.
---@field sensors BattlePerceptionStatus Condition of the sensor band.
---@field probe {kind: BattleProbeKind, range: integer, status: BattlePerceptionStatus}|nil Best installed active probe.
---@field radar {range: integer, status: BattlePerceptionStatus}|nil Anti-aircraft radar, if installed.
---@field running boolean Stopped units perceive nothing.
---@field text string The report printed by the sensor command.

---@alias BattleProbeKind "beagle"|"light"|"bloodhound"

---@alias BattleContactArc "front" | "right" | "rear" | "left"

---@class BattleContactView
---@field label string Battlefield label, lowercase for identified allies.
---@field coordinate BattleHexCoordinate
---@field elevation integer Current elevation.
---@field short_text string Plain compact biped contact row.
---@field verbose_text string Plain multiline C0 contact report.
---@field identified boolean Current terrain permits identification.
---@field weapon_arc BattleContactArc Observer torso direction; individual weapons may have different arcs.
---@field detection BattleDetectionChannel|nil How the observer currently perceives this contact; nil for clairvoyant-only views.
---@field status string Five visible condition columns; blank behind blocking terrain.
---@field target integer Acquired unit dbref.
---@field name string Chassis name, or "something" for unidentified signals.
---@field friendly boolean Identified and on the same team as observer.
---@field range BattleRange
---@field heading number Travel axis including lateral offset; reverse speed travels opposite this axis.
---@field speed number Current kph.

---Read acquired contacts the unit still perceives; no acquisition rolls.
---@param dbref integer Running observer unit dbref.
---@param preferences BattleContactPreferences? Optional inclusion filter; omitted lists all acquired contacts.
---@return BattleContactView[]
function btech_unit.contacts(dbref, preferences) end

---Select a current acquired target or clear selection with nil; requires the conscious assigned pilot.
---@param dbref integer
---@param pilot integer
---@param target integer?
---@return boolean
function btech_unit.lock(dbref, pilot, target) end

---@class BattleSpotterEvents
---@field events BattleSpotterEvent[] Independent requests in insertion order; detached inspection only.

---@class BattleSpotterEvent
---@field order integer Global order among active events.
---@field remaining integer Seconds until connection completion or maintenance.
---@field observer integer Observer unit dbref.
---@field positions BattlePoint[]? Captured shooter and observer coordinates during setup; nil for maintenance.

---@class BattleTagState
---@field target integer? Selected target; nil during recycle.
---@field remaining integer Lock/recycle seconds, zero through thirty.

---Illuminate an enemy within fifteen hexes, or stop with nil; requires working TAG and a ready timer.
---@param dbref integer
---@param pilot integer
---@param target integer?
---@return boolean
function btech_unit.tag(dbref, pilot, target) end

---Read current connected pilot gunnery under configured weapon-family rules; default six without one.
---@param dbref integer Unit dbref.
---@param weapon integer Zero-based weapon index.
---@return integer Signed skill target; no XP award or firing permission.
function btech_unit.gunnery(dbref, weapon) end

---@class BattleWeaponReadiness
---@field weapon string Conventional weapon kind.
---@field intact boolean
---@field ammunition integer Matching available salvos.
---@field recycle_remaining integer Simulation seconds.
---@field jammed boolean Ammunition-feed failure blocks firing and mode changes.
---@field spent boolean Self-contained salvo has already launched.
---@field posture_ready boolean Prone support and mounting restrictions.
---@field ready boolean Power, mechanical conditions, preparation and supply permit use; targeting and authority remain separate.

---@alias BattleVehicleSectionName "front"|"right"|"left"|"rear"|"turret"|"rotor"

---@class BattleWeaponInspection
---@field preferred_ammunition_section string|nil Canonical preferred ammunition section; fallback remains automatic.
---@field index integer Zero-based stable weapon number.
---@field name string Equipment display name.
---@field section BattleSectionName|BattleVehicleSectionName
---@field failure "jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam"|nil Temporary operational failure independent of physical integrity.
---@field rear_mount boolean
---@field one_shot boolean
---@field readiness BattleWeaponReadiness
---@field ammunition_mode BattleAmmunitionMode
---@field fire_mode BattleFireMode

---Inspect mounted weapons without acquiring targets or consuming dice; calling scripts own access policy.
---Rust extension retained under its descriptive name; the canonical weapons list follows the C contract.
---@param dbref integer
---@return BattleWeaponInspection[] Lua array positions start at one; use each entry's index to fire.
function btech_unit.weapon_states(dbref) end

---List mounted weapons in mounting order; an optional section restricts the result.
---@param unit DbRef|Object
---@param section? BattleSection Typed section constant from btech.unit.sections.
---@return BattleMountedWeapon[]
function btech_unit.weapons(unit, section) end

---Read a TIC's ordered weapon numbers; requires the conscious assigned pilot.
---@param dbref integer
---@param pilot integer
---@param group integer Zero-based group, 0 through 3.
---@return integer[]
function btech_unit.tic(dbref, pilot, group) end

---Edit persistent membership; add/remove require weapon numbers, clear omits them.
---@param dbref integer
---@param pilot integer
---@param group integer
---@param operation "add"|"remove"|"clear"
---@param weapons integer[]|nil
function btech_unit.tic_edit(dbref, pilot, group, operation, weapons) end

---Fire groups in ascending order using ordinary firing rules. Shot rejection continues;
---fall or shutdown stops firing. Callback failure restores the whole batch.
---@param dbref integer
---@param pilot integer
---@param groups integer[] Zero-based group numbers, 0 through 3.
---@param target integer|{x: integer, y: integer}|nil Explicit unit/coordinates, or cockpit selection when omitted.
---@return {group: integer, weapon: integer, report: table|nil, rejection: string|nil}[]
function btech_unit.tic_fire(dbref, pilot, groups, target) end

---Begin a four-second Mech heat cutoff toggle; requires the assigned conscious pilot.
---The configuration gate applies to new toggles; admitted transitions survive shutdown.
---@param dbref integer
---@param pilot integer
---@return boolean
function btech_unit.heatcutoff(dbref, pilot) end

---Toggle an MML between SRM and LRM ammunition; requires dedicated matching bins.
---SRMs use 3/6/9 range and two-point hits; LRMs use 7/14/21, minimum six, and five-point groups.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon number.
---@return BattleAmmunitionMode normal for SRM, mml_lrm for LRM.
function btech_unit.mml(dbref, pilot, weapon) end

---Toggle Extended Range ammunition on an eligible indirect launcher; requires matching bins.
---This reference ammunition marker does not change the weapon's range or damage profile.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.atmrange(dbref, pilot, weapon) end

---Toggle High Explosive ammunition using the same eligibility and saved selection rules.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.atmexplosive(dbref, pilot, weapon) end

---Power down a Gauss mount after recharge. Requires a running, mapped unit and conscious pilot.
---Persists across shutdown/restart, prevents firing and suppresses Gauss critical explosions.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon number.
---@return boolean
function btech_unit.disable(dbref, pilot, weapon) end

---Select an ammunition section, or clear with nil or '-'. Requires a conscious assigned
---pilot, map placement and an intact non-recycling ammunition weapon, but no reactor power.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon number.
---@param section string|nil Cockpit section name or abbreviation.
---@return boolean
function btech_unit.usebin(dbref, pilot, weapon, section) end

---Toggle automatic turret tracking; requires a surviving turret, map and conscious assigned pilot.
---The mode may be selected while stopped; actual tracking requires a running vehicle.
---@param dbref integer
---@param pilot integer
---@return boolean
function btech_unit.autoturret(dbref, pilot) end

---@class BattleAimModifiers
---@field self_target boolean Coolant self-application bypasses contact acquisition.
---@field indirect {spotter: integer, spotting: integer, movement: integer, target_lock: integer}|nil Observer contributions; perception then describes the spotter's view.
---@field gunnery integer
---@field distance number
---@field network_range {kind: "c3"|"c3i", distance: number, source: integer|nil}|nil Active command-network range; physical limits and firing visibility remain separate.
---@field range {bracket: string, modifier: integer}|nil
---@field attacker_movement integer
---@field attacker_water integer Plus one when firing at a unit from below the water surface.
---@field woods_cover integer Configured occupied-forest accuracy credit: zero, minus one or minus two.
---@field target_movement integer Movement contribution, including +1 for a VTOL with nonzero horizontal or vertical speed.
---@field dug_in integer Configured cover modifier, shared by Mech and vehicle attackers.
---@field orbital_drop integer Minus two while the target has an intact cocoon; zero after a breach.
---@field heat integer
---@field sensors integer
---@field control_damage integer Vehicle commander/sensor critical penalties.
---@field mounting_section integer
---@field targeting_computer integer Eligible computer fire: -1 normally, +3 for a selected section on a mobile target.
---@field aimed_section integer Head aim penalty: 7 against immobile Mechs, 25 against mobile Mechs; overrides computer assistance.
---@field beacon_accuracy integer Haywire interference and iNarc homing assistance.
---@field ammunition_accuracy integer Selected ammunition adjustment; LB-X cluster is -3 versus VTOLs, otherwise -1; Stinger is -3 versus flying VTOLs and -1 during orbital descent.
---@field targeting_mode integer Scenario tracking-mode adjustment, separate from installed computer equipment.
---@field weapon_accuracy integer Intrinsic accuracy adjustment; pulse lasers contribute -2, MRMs +1.
---@field weapon_damage integer Penalty from damaged focusing, ranging and other weapon components.
---@field target_lock integer
---@field perception {channel: BattleDetectionChannel|nil, direct_fire: boolean, modifier: integer}|nil Nil without a current contact; direct_fire is false behind blocking terrain.

---@class BattleSectionExposureReport
---@field cause "water"|"vacuum"
---@field section BattleSectionName
---@field reactor_explosion table|nil
---@field fall table|nil
---@field notices {unit: integer, text: string}[]

---@class BattleTacticalImpact
---@field impact table Ordered material damage, critical losses and exposures (BattleSectionExposureReport[]).
---@field pilot_injuries table[] Applied crew consequences.
---@field notices table[] Cockpit messages.
---@field balance table[] Applied balance checks and falls.
---@field flooding table[] Applied flooding consequences.

---@class BattleAmmunitionDraw
---@field bin_index integer Zero-based bin index.
---@field rounds integer

---@class BattleWeaponUse
---@field damage_penalty integer Energy damage lost to focusing damage.
---@field critical_failure "barrel"|"crystal"|"feed"|nil Component responsible for a failed launch.
---@field weapon string
---@field ammunition BattleAmmunitionDraw[] Actual live-bin expenditure.
---@field fire_mode BattleFireMode Effective mode after supply fallback.
---@field heat integer Already applied; do not add this heat again.
---@field gatling_damage integer|nil Supply-limited gatling damage before glancing.
---@field ammunition_mode BattleAmmunitionMode

---@class BattleSalvoGroup
---@field damage integer
---@field hit {section: BattleSectionName, rear_armor: boolean, through_armor_critical: boolean, crew_stun: boolean}
---@field impact table Ordered material phases, critical losses, exposures (BattleSectionExposureReport[]), dump_ignitions, plasma_heat rolls, searchlight_destroyed and remaining scenario effects.
---@field pilot_injuries table[] Applied tactical injuries and consciousness results.
---@field notices {unit: integer, text: string}[] Already staged by unit.fire.
---@field balance table[] Applied balance checks and any nested falls.
---@field flooding table[] Applied section flooding and any nested falls.

---@class BattleInfernoHit
---@field target integer
---@field missiles integer Surviving missiles after interception.
---@field burn_seconds integer Duration added before immersion.
---@field extinguished boolean
---@field notices BattleNotice[]

---@class BattleWoodlandImpact
---@field map integer
---@field coordinate BattleHexCoordinate
---@field effect table Ignition duration, replacement terrain, or no effect.
---@field notices BattleNotice[]

---@class BattleWoodsAbsorption
---@field damage_before integer Damage supplied to terrain before absorption: per shell for direct/burst fire, total for missiles after glancing cluster adjustment and interception.
---@field damage_after integer Remaining armor damage: minimum one per shell before glancing for direct/burst hits; whole-projectile totals may be zero.
---@field terrain BattleWoodlandImpact Committed ignition or clearing check.
---@field notices BattleNotice[] Ordered absorption and terrain feedback.

---@class BattleSalvoReport
---@field initial_woods BattleWoodsAbsorption|nil Nominal LBX terrain check before pellet counting and absorption.
---@field woods BattleWoodsAbsorption|nil Occupied-woods consequences for direct shells (including bursts) or missile/pellet armor damage, after missile interception.
---@field missiles_before_defense integer|nil Cluster hits before automatic defenses.
---@field cluster_roll integer|nil Original missile cluster roll; nil for direct non-missile hits.
---@field inferno BattleInfernoHit|nil Burning replaces armor damage.
---@field groups BattleSalvoGroup[]

---@class BattleCharacterValue
---@field value integer Trained skill level.
---@field experience integer Encoded earned levels and XP balance.
---@field last_used integer Last accepted award timestamp.

---@class BattleExperienceAward
---@field accepted boolean
---@field before BattleCharacterValue
---@field after BattleCharacterValue

---@class BattlePilotingCheck
---@field skill integer Base pilot skill target.
---@field damage integer Penalty from physical damage.
---@field cockpit integer Small cockpit construction penalty, independent of damage.
---@field situational integer Caller-supplied modifier.
---@field absent_character_pilot integer Penalty for an absent in-character pilot.
---@field target integer Total required roll.
---@field roll integer|nil No dice when already prone or unable to act.
---@field success boolean
---@field experience BattleExperienceAward|nil Accepted or rate-limited skill mutation for XP-awarding callers.

---@class BattleRecoilReport
---@field experience_messages BattleChannelMessage[] Accepted recoil XP diagnostics published with the shot.
---@field check BattlePilotingCheck
---@field fall BattleFallReport|nil

---@class BattleAmsReport
---@field weapon_index integer Zero-based defensive weapon index.
---@field ammunition_bin integer Selected normal-ammunition bin.
---@field roll integer Interception capacity before rack and cluster limits.
---@field ammunition_spent integer May be less than interception capacity.
---@field shot_down integer Actual intercepted hits after the missile meets its base target number.

---@alias BattleBeaconKind "narc"|"homing"|"haywire"|"ecm"
---@class BattleNarcReport
---@field kind BattleBeaconKind
---@field notices BattleNotice[] Cockpit effects from the hit-location roll.
---@field hit boolean Whether the beacon met the full attack target.
---@field intercepted boolean Whether AMS intercepted the pod.
---@field section BattleSection|BattleVehicleSectionName|nil Surviving attachment section.
---@field rear boolean Rear-facing attachment notice.

---@class BattleShotReport
---@field launch_notices BattleNotice[] Cocoon opening feedback before target consequences.
---@field coordinate {x: integer, y: integer}|nil Coordinate-directed shot; target identifies the selected occupant.
---@field experience_messages table[] Accepted spotting/artillery awards, including misses.
---@field streak_confused boolean Angel interference disables Streak homing.
---@field narc BattleNarcReport|nil Normal beacon outcome; explosive pods use salvo damage.
---@field ams BattleAmsReport|nil Automatic defense activation; absent for missile rolls below base target number.
---@field ammunition_warning string|nil Pre-expenditure warning staged with the shot.
---@field shooter integer
---@field target integer
---@field weapon_index integer Zero-based stable weapon number.
---@field aim BattleAimModifiers
---@field target_number integer|nil Ordinary aim subtotal; nil beyond physical range.
---@field roll integer
---@field glancing boolean
---@field recoil BattleRecoilReport|nil Moving Heavy Gauss control check and fall.
---@field jammed boolean Recoverable ammunition-feed failure without expenditure.
---@field loader_destroyed boolean Permanent mount loss from loader failure or propellant ignition.
---@field propellant_roll integer|nil Second caseless roll; eight or more ignites propellant.
---@field misload BattleTacticalImpact|nil Applied misload or propellant ignition damage.
---@field launched boolean False for failed Streak lock: no heat/ammo expenditure, but weapon recycles.
---@field expenditure BattleWeaponUse
---@field salvo {kind: 'mech'|'vehicle'|'swarm', report: table}|nil Target-specific damage; nil on a miss or a heat-mode hit.
---@field heat_transfer integer Heat already added to the target, zero unless a heat-mode shot hits.
---@field thermal_woods BattleWoodsAbsorption|nil Terrain effects and feedback preceding thermal transfer; heat/cooling strength remains unchanged.
---@field missed_terrain BattleWoodlandImpact|nil Incidental terrain check after a launched non-missile miss, independent of woods damage configuration.
---@field cooling number? Coolant reduction applied to stored heat, including temporary negative credit.

---@class BattleVehicleShotReport
---@field experience_messages BattleChannelMessage[] Accepted spotting/artillery awards, including misses.
---@field coordinate {x: integer, y: integer}|nil Occupied-hex shot; target identifies the selected occupant.
---@field shooter integer
---@field target integer
---@field weapon_index integer Zero-based stable weapon number.
---@field aim BattleAimModifiers
---@field streak_confused boolean
---@field launch BattleVehicleLaunch
---@field ams BattleAmsReport|nil
---@field narc BattleNarcReport|nil Beacon attachment or interception; vehicle sections use their own names.
---@field cooling number|nil Coolant removed from target stored heat.
---@field heat_transfer integer Direct flamer heat added to the target, otherwise zero.
---@field thermal_woods BattleWoodsAbsorption|nil Terrain effects and feedback preceding thermal transfer; heat/cooling strength remains unchanged.
---@field missed_terrain BattleWoodlandImpact|nil Incidental terrain check after a launched non-missile miss, independent of woods damage configuration.
---@field salvo {kind: 'mech'|'vehicle'|'swarm', report: table}|nil Target-specific ordered damage groups.

---@class BattleVehicleInfernoHit
---@field missiles integer Surviving missiles after clustering and interception.
---@field explosion_roll integer|nil Standard mobile-vehicle heat check.
---@field burn_seconds integer Jelly duration added to a stationary unit.
---@field damage table[] Ordered initial section fire damage.
---@field explosion table|nil Completed heat catastrophe.
---@field notices BattleNotice[] Already staged by firing.
---@field broadcasts BattleNotice[] Raw damage broadcasts handled by firing.

---@class BattleVehicleSalvoReport
---@field initial_woods BattleWoodsAbsorption|nil Nominal LBX terrain check before pellet counting and absorption.
---@field woods BattleWoodsAbsorption|nil Occupied-woods consequences for direct shells (including bursts) or missile/pellet armor damage, after missile interception.
---@field experience table[] Per-packet optional pre-impact XP awards.
---@field experience_messages table[] Ordered XP channel diagnostics.
---@field cluster_roll integer|nil
---@field missiles_before_defense integer|nil
---@field groups table[] Located conventional damage packets.
---@field inferno BattleVehicleInfernoHit|nil Dedicated vehicle inferno outcome.

---@class BattleVehicleLaunch
---@field ammunition_warning string|nil Pre-expenditure warning staged with the shot.
---@field launch_notices BattleNotice[] Cocoon opening feedback before target consequences.
---@field roll integer
---@field hit boolean Launch classification; missile near misses may have no target effects.
---@field glancing boolean Tactical missile shots use the base target-number boundary.
---@field loader_destroyed boolean
---@field jammed boolean
---@field propellant_roll integer|nil
---@field misload table|nil Shooter internal damage and critical consequences.
---@field expenditure table Weapon, ammunition, fire mode, spent rounds, recycle and launched status.

---@class BattleArtilleryLaunchReport
---@field launch_notices BattleNotice[] Cocoon opening feedback before target consequences.
---@field shooter integer
---@field map integer
---@field coordinate {x: integer, y: integer}
---@field weapon_index integer
---@field aim {target_number: integer, maximum_range: integer, range: string}
---@field roll integer
---@field hit boolean
---@field launched boolean
---@field jammed boolean
---@field loader_destroyed boolean
---@field propellant_roll integer|nil
---@field expenditure BattleWeaponUse
---@field misload {kind: "mech"|"vehicle", report: BattleTacticalImpact|BattleVehicleInternalDamage}|nil
---@field ammunition_warning string|nil
---@field queued_shot integer|nil Persistent map queue ordinal, present after launch.

---@class BattleHexShotReport
---@field launch_notices BattleNotice[] Cocoon opening feedback before target consequences.
---@field shooter integer
---@field map integer
---@field coordinate {x: integer, y: integer}
---@field weapon_index integer
---@field aim BattleHexAimModifiers
---@field target_number integer|nil
---@field roll integer
---@field hit boolean
---@field launched boolean
---@field jammed boolean
---@field loader_destroyed boolean
---@field propellant_roll integer|nil
---@field expenditure BattleWeaponUse
---@field misload {kind: "mech"|"vehicle", report: BattleTacticalImpact|BattleVehicleInternalDamage}|nil
---@field ammunition_warning string|nil
---@field cluster_roll integer|nil
---@field terrain table[] Applied woodland effects and captured notices.
---@field surfaces table[] Structural rolls, optional fracture/falls, and notices.
---@field buildings table[] Building identity, actual damage, remaining integrity and notices.
---@field recoil BattleRecoilReport|nil

---Fire under configured tactical rules and stage cockpit notices in the current callback transaction.
---Requires the conscious assigned pilot; the calling script owns authority to act as that pilot.
---An omitted target uses cockpit selection, including automatic coolant self-selection.
---An IDF observer takes precedence when the firer has no unit lock, even with explicit arguments.
---Explicit coordinates select an occupant for conventional weapons or use terrain effects when empty.
---Artillery always uses coordinates and queues its impact. Explicit requests do not change saved locks.
---Missile effects require the base target number even with near-miss glancing enabled.
---Such a near miss spends its launch and aimed preparation, but starts no AMS, pod effect or Swarm flight.
---Results are detached; optional fields use nil.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon number from unit.weapons.
---@param target integer|{x: integer, y: integer}|nil Explicit target leaves the selected lock unchanged.
---@return BattleShotReport|BattleVehicleShotReport|BattleHexShotReport|BattleArtilleryLaunchReport
function btech_unit.fire(dbref, pilot, weapon, target) end

---@alias BattleAimSelection {class: "mech", section: string}|{class: "ground_vehicle"|"vtol", section: string}

---Select a section of the current unit target using its anatomical aliases.
---Requires a running unit and its conscious assigned pilot. Nil or "-" clears without a lock.
---The saved class and section persist across lock changes and shutdown. Returned values are detached.
---@param dbref integer
---@param pilot integer
---@param section string|nil
---@return BattleAimSelection|nil
function btech_unit.target(dbref, pilot, section) end

---Read the saved anatomical preference without requiring a running unit or a current target.
---@param dbref integer
---@return BattleAimSelection|nil
function btech_unit.aimed_section(dbref) end

---@class BattleSightReport
---@field shooter integer
---@field weapon_index integer
---@field weapon string
---@field target integer|nil
---@field coordinate BattleHexCoordinate|nil
---@field aim BattleAimModifiers|BattleHexAimModifiers|BattleArtilleryAim
---@field target_number integer|nil Nil when out of range.
---@field roll integer Attack dice consumed without launching.
---@field gatling_roll integer|nil Preparation intensity, without an ammunition cap.
---@field partial_cover boolean

---Sight a weapon using ordinary target selection and aim, without firing or revealing cover.
---Consumes preparation and attack dice; ignores ammunition, recycling and feed jams.
---Requires an intact offensive mount and the conscious assigned pilot of a running unit.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@param target integer|BattleHexCoordinate|nil Omitted target uses cockpit selection.
---@return BattleSightReport
function btech_unit.sight(dbref, pilot, weapon, target) end

---Rotate one step left/right or center the torso; stages the native cockpit message.
---@param dbref integer
---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
---@param direction 'left'|'right'|'center' Case-insensitive; l/r/c aliases are accepted.
---@return boolean
function btech_unit.rottorso(dbref, pilot, direction) end

---Schedule a five-second searchlight toggle; repeated calls preserve the pending switch.
---@param dbref integer
---@param pilot integer
---@return boolean
function btech_unit.slite(dbref, pilot) end

---Toggle forward/backward arms on a capable standing, running chassis.
---@param dbref integer
---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
---@return boolean
function btech_unit.fliparms(dbref, pilot) end

---Attempt a jump; a failed stagger check falls instead of launching, within the callback transaction.
---A completed check survives later destination rejection; an enclosing callback failure still rolls back.
---@param dbref integer
---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
---@param bearing integer Compass degrees.
---@param range number Positive range in hex heights, snapped to a destination hex center.
---@return boolean Accepted attempt; inspect flight state to distinguish launch from a stagger fall.
function btech_unit.jump(dbref, pilot, bearing, range) end

---Attempt early jump landing or VTOL touchdown at the current point; cancel a queued VTOL launch.
--- Character XP, injuries and crew evacuation commit together; failures restore the whole landing.
---@param dbref integer
---@param pilot integer
---@return boolean
function btech_unit.land(dbref, pilot) end

---Begin an eighteen-second hangar entry; current route, eligibility and locks are rechecked at expiry.
---@param dbref integer
---@param pilot integer
---@param direction string? One-byte direction; omitted or longer selector uses the first entrance.
---@return boolean admitted False when the enter lock denies entry.
function btech_unit.enterbase(dbref, pilot, direction) end

---Queue VTOL takeoff using configured fuel rules and stage a cockpit confirmation.
---@param dbref integer
---@param pilot integer
---@param delay integer? Extra launch seconds, 0..65535; nonzero requires a wizard.
---@return boolean
function btech_unit.takeoff(dbref, pilot, delay) end

---Read or set VTOL vertical speed using configured fuel rules and the shared velocity budget.
---@param dbref integer
---@param pilot integer
---@param kph number? Omit for current speed; positive climbs, negative descends.
---@return boolean|number
function btech_unit.vertical(dbref, pilot, kph) end

---Attempt to stand, staging fall and terrain-break notices in the callback transaction.
---@param dbref integer
---@param pilot integer
---@param mode? 'normal'|'anyway'|'careful'
---@return table attempt Contains check, optional fall, rise/retry timer and ordered notices.
function btech_unit.stand(dbref, pilot, mode) end

---Drop prone; fast travel can require a control roll and cause ordinary fall damage.
---@param dbref integer
---@param pilot integer
---@return table report Contains optional check/fall, flooding, stepping mines and ordered notices.
function btech_unit.prone(dbref, pilot) end

---Begin hiding in forest, mountains or rough terrain; requires camouflage equipment or a wizard pilot.
---Hostile acquired contacts or leaving the ground stop preparation. Hex crossings and firing cancel it.
---@param dbref integer Supported unit dbref.
---@param pilot integer Assigned pilot.
---@return boolean accepted Timer and cockpit feedback commit with the callback.
function btech_unit.hide(dbref, pilot) end

---@class BattleWeaponValues
---@field recycle_seconds integer Effective runtime recycle time, from 1 through 127 seconds.
---@field battle_value integer Effective runtime Battle Value, from 0 through 2147483647.

local btech_weapon = {}

---Read detached effective values using a canonical or manufacturer-qualified weapon name.
---Examples: IS.MediumLaser or Magna.IS.MediumLaser; exact names ignore ASCII case.
---@param name string
---@return BattleWeaponValues
function btech_weapon.settings(name) end

---Wizard-only runtime override. Existing countdowns are unchanged; restart restores catalogue defaults.
---@param actor integer
---@param name string
---@param seconds integer From 1 through 127.
---@return BattleWeaponValues
function btech_weapon.set_recycle(actor, name, seconds) end

---Wizard-only runtime override used by valuation and Battle Value XP. Restart restores catalogue defaults.
---@param actor integer
---@param name string
---@param value integer From 0 through 2147483647.
---@return BattleWeaponValues
function btech_weapon.set_battle_value(actor, name, value) end

---@class BattleInventoryEntry
---@field part_id integer Stable game-directory part identifier.
---@field brand_id integer Manufacturer identifier, zero through five.
---@field quantity integer Positive stock quantity, at most 2147483647.

---@class BattlePart
---@field part_id integer Stable inventory identifier.
---@field name string Canonical stock name.
---@field kind "weapon"|"ammunition"|"component"|"commodity"|"bomb"
---@field mass integer Catalogue mass in 1/1024 tons; loose bomb stock uses four times this value.

local btech_inventory = {}

---Wizard signed adjustment of one catalogue match, ordered by long name after exact matching.
---Positive counts cap at 50000; negative counts are not capped. Zero succeeds without matching.
---Stock, load correction and diagnostics roll back together. No match returns false.
---@param actor integer
---@param object integer
---@param pattern string Fewer than 2048 bytes.
---@param quantity integer Signed 32-bit count.
---@return boolean
function btech_inventory.add_stores(actor, object, pattern, quantity) end

---Wizard catalogue-based addition; amount is capped at 50000 per match.
---Wizards other than GOD may select at most 20 catalogue entries. Stock and diagnostics commit together.
---@param actor integer
---@param object integer
---@param pattern string Exact abbreviation/full name, then wildcard; may match absent stock.
---@param quantity integer Positive requested quantity per match.
---@return BattleCargoRow[] Requested changes after the request cap.
function btech_inventory.add(actor, object, pattern, quantity) end

---Wizard removal floors stock at zero; reports and diagnostics retain the capped requested amount.
---@param actor integer
---@param object integer
---@param pattern string
---@param quantity integer Positive requested quantity per match.
---@return BattleCargoRow[]
function btech_inventory.remove(actor, object, pattern, quantity) end

---Wizard reset removes every stock row and emits one reset record, including for an empty holder.
---@param actor integer
---@param object integer
function btech_inventory.clear(actor, object) end

---@class BattleInventoryCleanup
---@field original_entries integer
---@field new_entries integer
---@field items integer

---Wizard cleanup of loose stock, removing structural placeholders and unknown identifiers.
---Preserves installed equipment; reconciles carrying load. Callback failure restores inventory.
---@param actor integer
---@param object integer
---@return BattleInventoryCleanup
function btech_inventory.fix(actor, object) end

---Describe stock by exact name or stored identifier; names ignore ASCII case.
---@param part string|integer
---@return BattlePart
function btech_inventory.part(part) end

---Physical loose-stock mass in 1/1024 tons, before chassis cargo discounts.
---@param object integer
---@return integer
function btech_inventory.mass(object) end

---Wizard stock correction by exact part name, sharing validation and callback rollback with set.
---@param actor integer
---@param object integer
---@param name string
---@param brand integer From zero through five.
---@param quantity integer From zero through 2147483647.
function btech_inventory.set_named(actor, object, name, brand, quantity) end

---Read an object's detached, ordered loose-parts stock in a callback.
---@param object integer
---@return BattleInventoryEntry[]
function btech_inventory.read(object) end

---Wizard stock correction using stored identifiers; zero quantity removes the entry.
---Stock, immediate load correction and MechEconInfo diagnostics participate in callback rollback.
---Unchanged quantities emit no record. Does not install equipment or perform cargo loading.
---@param actor integer
---@param object integer
---@param part integer Nonnegative signed-32-bit identifier.
---@param brand integer From zero through five.
---@param quantity integer From zero through 2147483647.
function btech_inventory.set(actor, object, part, brand, quantity) end

---@class BattleCargoRow: BattleInventoryEntry
---@field name string Stock display name, including a known weapon manufacturer when available.

local btech_cargo = {}

---Read detached stock in the actor's current location. Requires cargo commands enabled.
---@param actor integer
---@param pattern? string Case-insensitive stock name, numeric part ID, or wildcard pattern.
---@return BattleCargoRow[]
function btech_cargo.manifest(actor, pattern) end

---Read hangar stock from a running unit at the configured loading point.
---@param actor integer
---@param pattern? string
---@return BattleCargoRow[]
function btech_cargo.stores(actor, pattern) end

---Load matching hangar stock into a stationary, running CargoTech unit.
---Exact abbreviations precede exact catalogue names, then wildcard names; selection is independent of available stock.
---Transfers, throttle correction and MechEconInfo diagnostics are atomic and participate in callback rollback.
---@param actor integer
---@param pattern string
---@param quantity integer Positive request per matched row, capped at 50000 and available stock.
---@return BattleCargoRow[] Transferred quantities.
function btech_cargo.load(actor, pattern, quantity) end

---Unload matching CargoTech stock onto the current map; startup and loading-point checks do not apply.
---@param actor integer
---@param pattern string
---@param quantity integer Positive request per matched row, capped at 50000 and available stock.
---@return BattleCargoRow[] Transferred quantities.
function btech_cargo.unload(actor, pattern, quantity) end

---@class btech.database
local btech_database = {}

---Wizard runtime diagnostics.
local btech_runtime = {}

---@class BattleTechInspection
---@field database btech.database Explicit world checkpoints.
---@field cargo table Cockpit stock reports and transfers.
---@field inventory table Shared loose-parts stock.
---@field weapon table Runtime weapon settings.
---@field character table
---@field template table
---@field map table
---@field player table Saved player preferences.
---@field unit table
---@field parts table Registered part catalogue and stock queries.
---@field repair table Immediate repair requests and technician scheduling.
---@field system table World event telemetry.
---@field autopilot BtechAutopilotAPI Lua control of unit-attached ground autopilots.
---@field tactical BtechTacticalAPI Filtered group observations and atomic intentions.
---@field errors table Structured btech error-code tree from mux.error.code_tree('btech').
btech = {
  runtime = btech_runtime,
  database = btech_database,
  cargo = btech_cargo,
  inventory = btech_inventory,
  weapon = btech_weapon,
  player = btech_player,
  character = btech_character,
  template = btech_template,
  map = btech_map,
  unit = btech_unit,
}

---Set the unit's downhill cliff preference; requires its present pilot.
---@param dbref integer
---@param player integer
---@param enabled boolean
---@return boolean
function btech_unit.auto_fall(dbref, player, enabled) end

---Toggle an intact, recycled flamer between damage and heat-transfer modes; stages cockpit notices.
---@param dbref integer
---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
---@param weapon integer Zero-based weapon index.
---@return BattleFireMode
function btech_unit.flamerheat(dbref, pilot, weapon) end

---Toggle an intact, recycled LB-X autocannon between slug and cluster ammunition; stages cockpit notices.
---@param dbref integer
---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.lbx(dbref, pilot, weapon) end

---Toggle an intact recycled artillery launcher between normal and cluster rounds; rejects smoke/mine selection.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.cluster(dbref, pilot, weapon) end

---@class BattleAmmunitionAdjustment
---@field location BattleCriticalLocation
---@field supplied integer Authored initial quantity.
---@field normalized integer Initial quantity after construction normalization.
---@field inferred_half_ton boolean Construction would infer a half-ton bin.

---@class BattleTemplateCheck
---@field chassis "biped"|"quad"|nil Parsed anatomy, independent of simulation readiness.
---@field name string
---@field reference string
---@field constructible boolean Passes currently implemented biped construction checks.
---@field rejection string|nil First construction failure; nil for a constructible template.
---@field weapons integer Resolved weapon count on success; zero on rejection.
---@field ammunition_bins integer Resolved bin count on success; zero on rejection.
---@field ammunition_adjustments BattleAmmunitionAdjustment[] Changes on successful construction.

---Preview construction without registering a unit or modifying the source asset.
---@param name string Bounded asset name from the configured mech directory.
---@return BattleTemplateCheck
function btech_template.check(name) end

---@class BattleArtemisController
---@field location {section: BattleSectionName|BattleVehicleSectionName, slot: integer} Zero-based controller position.
---@field link integer One-based template launcher slot; zero is unassigned.
---@field weapon_indices integer[] Zero-based missile mounts matching the link.
---@field operational boolean Controller is available under the unit’s equipment damage rules.

---Toggle Artemis-compatible ammunition; requires a live linked controller and a recycled missile launcher.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.artemis(dbref, pilot, weapon) end

---@class BattleUnjam
---@field weapon_index integer Zero-based weapon number.
---@field remaining integer Committed seconds remaining, 1 through 60.

---Begin timed feed recovery; stages cockpit feedback and participates in callback rollback.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return boolean
function btech_unit.unjam(dbref, pilot, weapon) end

---Toggle hotloading on a supported recycled indirect-fire launcher; stages cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleFireMode
function btech_unit.hotload(dbref, pilot, weapon) end

---Toggle one- or two-round Ultra autocannon firing; stages cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleFireMode
function btech_unit.ultra(dbref, pilot, weapon) end

---Toggle rapid two-round conventional or light autocannon firing.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleFireMode
function btech_unit.rapidfire(dbref, pilot, weapon) end

---Set rotary burst length; repeated selection stays enabled and returns false.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@param rounds? 1|2|4|6 Defaults to one.
---@return boolean changed
function btech_unit.rac(dbref, pilot, weapon, rounds) end

---Toggle gatling machine-gun fire, with cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleFireMode
function btech_unit.gattling(dbref, pilot, weapon) end

---Toggle armor-piercing autocannon ammunition with cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.armorpiercing(dbref, pilot, weapon) end

---Toggle caseless autocannon ammunition with cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.caseless(dbref, pilot, weapon) end

---Toggle incendiary autocannon ammunition with cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.incendiary(dbref, pilot, weapon) end

---Toggle inferno missile ammunition with cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.inferno(dbref, pilot, weapon) end

---Toggle Precision autocannon ammunition with cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.precision(dbref, pilot, weapon) end

---Toggle Flechette autocannon ammunition with cockpit feedback.
---@param dbref integer
---@param pilot integer
---@param weapon integer Zero-based weapon index.
---@return BattleAmmunitionMode
function btech_unit.flechette(dbref, pilot, weapon) end

---Set the assigned pilot's unit illumination-warning preference.
---@param dbref integer
---@param player integer
---@param enabled boolean
---@return boolean
function btech_unit.searchlight_warning(dbref, player, enabled) end

---Set the assigned pilot's armor warning preference.
---@param dbref integer
---@param player integer
---@param enabled boolean
---@return boolean
function btech_unit.armor_warning(dbref, player, enabled) end

---Set the assigned pilot's ammunition warning preference.
---@param dbref integer
---@param player integer
---@param enabled boolean
---@return boolean
function btech_unit.ammunition_warning(dbref, player, enabled) end

---Set MechWarrior safety; requires the assigned pilot in the cockpit.
---@param dbref integer
---@param player integer
---@param enabled boolean
---@return boolean success
function btech_unit.mw_safety(dbref, player, enabled) end

---Set the retained BTHDebug preference; requires the assigned pilot in the cockpit.
---@param dbref integer
---@param player integer
---@param enabled boolean
---@return boolean success
function btech_unit.bth_debug(dbref, player, enabled) end

---Set the assigned pilot's friendly-fire safety. Coolant guns are exempt.
---@param dbref integer
---@param player integer
---@param enabled boolean
---@return boolean
function btech_unit.friendly_fire_safety(dbref, player, enabled) end

---Read-only cockpit status as styled-text source with escaped literal fields. Select armor, info, weapons, heat, short or AIWHS. N/NW select the compact export.
---@param dbref integer
---@param options string?
---@return string
function btech_unit.status(dbref, options) end

---@alias BattleEquipmentCondition 'empty'|'operational'|'damaged'|'disabled'|'broken'|'destroyed'|'jammed'|'shorted'|'ammo_jam'

---@class BattleWeaponDamageEffects
---@field moderate integer General accuracy penalty.
---@field ranging integer Accuracy penalty beyond short range.
---@field heat integer Additional firing heat.
---@field damage integer Energy damage reduction.
---@field explosion integer Nonzero count explodes on an attack roll of count plus one or less.
---@field jam integer Nonzero count jams on an attack roll of count plus one or less.
---@field feed_locked boolean Prevents changing ammunition modes.

---@class BattleWeaponDiagnostic
---@field index integer Zero-based installed mount number, including destroyed mounts.
---@field weapon string Catalogue weapon identifier (snake case).
---@field section string Chassis-specific location name.
---@field condition BattleEquipmentCondition
---@field damaged_slots integer
---@field destroyed_slots integer
---@field disabled_slots integer
---@field effects BattleWeaponDamageEffects Existing firing penalties, without recomputation in Lua.
---@field preferred_ammunition_section string?

---Inspect durable equipment condition; empty ammunition, shutdown and recycle do not imply damage.
---Requires a trusted callback transaction. Returned rows are detached from saved unit state.
---@param dbref integer
---@return BattleWeaponDiagnostic[]
function btech_unit.weapon_diagnostics(dbref) end

---@class BattleWeaponSpecification
---@field weapon BattleWeapon Catalogue weapon identifier (snake case).
---@field ammunition BattleAmmunitionMode MMLs have separate normal (SRM) and mml_lrm rows.
---@field heat integer
---@field damage integer
---@field minimum_range integer
---@field short_range integer
---@field medium_range integer
---@field long_range integer Effective range in hexes, including artillery map-sheet conversion.
---@field extended_range integer? Present when extended range is configured.
---@field recycle_seconds integer Effective runtime value for new activations.

---Inspect distinct installed weapon types in first-installation order, including destroyed mounts.
---Requires a trusted callback transaction. Range columns follow the server's extended-range setting.
---@param dbref integer
---@return BattleWeaponSpecification[]
function btech_unit.weapon_specifications(dbref) end

---@class BattleCriticalInspection
---@field slot integer Zero-based physical slot; native labels add one.
---@field equipment string Resolved display name, including configured manufacturer and bin mode.
---@field condition BattleEquipmentCondition
---@field weapon_index integer? Stable zero-based mount index, including split extensions.
---@field ammunition_index integer? Zero-based bin index.
---@field ammunition_remaining integer? Saved bin quantity; native text hides it when unavailable.
---@field ammunition_capacity integer? Installed bin capacity, including special rounds and half tons.
---@field brand integer? Authored quality; split slots use their parent weapon's brand.
---@field rear_mount boolean
---@field one_shot boolean
---@field spent boolean
---@field controls_slot integer? Authored Artemis display label, already one-based.

---@class BattleCriticalReport
---@field section string Stable Mech or vehicle section identity.
---@field name string Chassis-specific display heading.
---@field slots BattleCriticalInspection[] All six or twelve physical slots, including empty ones.

---Inspect equipment by cockpit section alias. The trusted query requires a callback transaction.
---Rows are detached from saved state; native CRITSTATUS separately requires a conscious assigned pilot.
---@param dbref integer
---@param section string
---@return BattleCriticalReport
function btech_unit.criticals(dbref, section) end

---Attempt a biped kick; rolls back damage, falls, recovery and notices with the callback.
---@param dbref integer
---@param pilot integer
---@param leg? 'left'|'right' Defaults to right.
---@param target? integer Defaults to the selected target; explicit targets require acquisition.
---@return table report Attack profile, roll, hit, glancing, impact, balance, fall and notices.
function btech_unit.kick(dbref, pilot, leg, target) end

---Attempt one or both arms in left-to-right order. Default selection is both.
---Unavailable arms are reported separately when another arm attacks; an entirely rejected action raises an error.
---Impact failures and callback aborts roll back the complete action and staged messages.
---@param dbref integer
---@param pilot integer
---@param arms? 'left'|'right'|'both'
---@param target? integer Defaults to the selected target; explicit targets require acquisition.
---@return table report Ordered attacks, per-arm rejections and notices. Each attack includes its profile and optional impact.
function btech_unit.punch(dbref, pilot, arms, target) end

---Attempt a leg trip. A hit forces target balance; a miss has no balance check. No direct impact damage.
---Both legs and hips must be usable; the target must be standing and not rising.
---@param dbref integer
---@param pilot integer
---@param leg? 'left'|'right' Defaults to right.
---@param target? integer Defaults to selected target; explicit targets require acquisition.
---@return table report Attack profile, roll, hit, glancing, optional balance/fall, and notices.
function btech_unit.trip(dbref, pilot, leg, target) end

---Attempt an axe swing. Default selection tries equipped arms left first; an accepted swing blocks the other arm through recovery.
---@param dbref integer
---@param pilot integer
---@param arms? 'left'|'right'|'both'
---@param target? integer Defaults to selected target; explicit targets require acquisition.
---@return table report Ordered attacks, per-arm rejections and transactional notices.
function btech_unit.axe(dbref, pilot, arms, target) end

---Attempt a sword swing with the same selection and transaction rules as axe.
---@param dbref integer
---@param pilot integer
---@param arms? 'left'|'right'|'both'
---@param target? integer
---@return table report
function btech_unit.sword(dbref, pilot, arms, target) end

---Attempt a mace swing. A missed swing requires an attacker piloting check with a +2 modifier.
---@param dbref integer
---@param pilot integer
---@param arms? 'left'|'right'|'both'
---@param target? integer
---@return table report Ordered attacks, per-arm rejections and transactional notices.
function btech_unit.mace(dbref, pilot, arms, target) end

---Attempt a dual-saw attack; seven operational parts required, fixed seven base damage without TSM boost.
---@param dbref integer
---@param pilot integer
---@param arms? 'left'|'right'|'both'
---@param target? integer
---@return table report Ordered attacks, per-arm rejections and transactional notices.
function btech_unit.saw(dbref, pilot, arms, target) end

---Attempt claw attacks, left then right by default; each accepted arm starts its own recovery.
---@param dbref integer
---@param pilot integer
---@param arms? 'left'|'right'|'both'
---@param target? integer
---@return table report Ordered attacks, per-arm rejections and transactional notices.
function btech_unit.claw(dbref, pilot, arms, target) end

---Grab a tree in a selected arm (left first by default), or drop it with '-'.
---@param dbref integer
---@param pilot integer
---@param arm? 'left'|'right'|'-'
---@return table notices
function btech_unit.grabclub(dbref, pilot, arm) end

---Swing a club using both arms; a carried tree shatters on a hit. Forest terrain supplies an immediate tree.
---@param dbref integer
---@param pilot integer
---@param target? integer
---@return table report Physical attack, damage, recovery and notices.
function btech_unit.club(dbref, pilot, target) end

---Select a charge target without starting movement. Nil uses the current target; '-' cancels.
---@param dbref integer
---@param pilot integer
---@param target integer|"-"|nil
---@return table[] notices
function btech_unit.charge(dbref, pilot, target) end

---Attempt a DFA jump using the shared pre-launch stagger check. Nil uses the current target lock.
---@param dbref integer
---@param pilot integer
---@param target? integer
---@return boolean Accepted attempt; a stagger failure can prevent launch.
function btech_unit.dfa(dbref, pilot, target) end

---Toggle automatic anti-missile defense, or set an explicit enabled state.
---@param dbref integer
---@param pilot integer
---@param enabled boolean|nil
---@return boolean enabled
function btech_unit.ams(dbref, pilot, enabled) end

---Toggle Narc-compatible ammunition on a missile weapon.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.narc(dbref, pilot, weapon) end

---Toggle explosive ammunition on a Narc launcher.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.explosive(dbref, pilot, weapon) end

---@alias BattleElectronicMode "off"|"ecm"|"eccm"
---@class BattleElectronicField
---@field protected boolean
---@field angel_protected boolean
---@field disturbed boolean
---@field angel_disturbed boolean
---@field countered boolean
---@class BattleElectronics
---@field guardian BattleElectronicMode
---@field angel BattleElectronicMode
---@field field BattleElectronicField

---Toggle the corresponding suite mode inside the callback transaction.
---@param dbref integer
---@param pilot integer
---@return BattleElectronicMode
function btech_unit.ecm(dbref, pilot) end

---Toggle the corresponding suite mode inside the callback transaction.
---@param dbref integer
---@param pilot integer
---@return BattleElectronicMode
function btech_unit.eccm(dbref, pilot) end

---Toggle the corresponding suite mode inside the callback transaction.
---@param dbref integer
---@param pilot integer
---@return BattleElectronicMode
function btech_unit.angelecm(dbref, pilot) end

---Toggle the corresponding suite mode inside the callback transaction.
---@param dbref integer
---@param pilot integer
---@return BattleElectronicMode
function btech_unit.angeleccm(dbref, pilot) end

---Select iNarc homing (-), explosive (X), haywire (Y), ECM (E), or Nemesis (Z) ammunition.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@param selector? string
---@return BattleAmmunitionMode
function btech_unit.inarc(dbref, pilot, weapon, selector) end

---@class BattlePodRow
---@field section BattleSection|BattleVehicleSectionName
---@field destroyed boolean
---@field kinds BattleBeaconKind[]
---@class BattlePodRemoval
---@field section BattleSection
---@field kind BattleBeaconKind
---@field arm "left"|"right"
---@field target_number integer
---@field roll integer
---@field removed boolean
---@field self_damage integer
---@field impact BattleTacticalImpact|nil
---@field notices BattleNotice[]

---Inspect pod effects on all sections, or return an empty list when none are attached.
---@param dbref integer
---@param pilot integer
---@return BattlePodRow[]
function btech_unit.pods(dbref, pilot) end

---Swat one iNarc pod; a failed attempt deals self-damage. H selects homing, Y haywire, E ECM.
---@param dbref integer
---@param pilot integer
---@param section string
---@param kind string
---@return BattlePodRemoval
function btech_unit.removepod(dbref, pilot, section, kind) end

---Begin the vehicle crew's saved 60-second action; ordinary Narc pods remain attached.
---The assigned conscious pilot must be running and placed, with no forward motion or conflicting crew action. VTOLs must be landed (launch preparation is still landed).
---@param dbref integer
---@param pilot integer
---@return boolean started
function btech_unit.removepods(dbref, pilot) end

---@class BattleSignatureTransition
---@field enabled boolean
---@field remaining integer
---@class BattleSignatureState
---@field enabled boolean
---@field pending BattleSignatureTransition|nil

---Request a thirty-second stealth armor switch inside the callback transaction.
---@param dbref integer
---@param pilot integer
---@return boolean
function btech_unit.stealth(dbref, pilot) end

---Request a thirty-second null signature system switch inside the callback transaction.
---@param dbref integer
---@param pilot integer
---@return boolean
function btech_unit.nss(dbref, pilot) end

---Toggle semi-guided ammunition on a supported recycled missile launcher.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.sguided(dbref, pilot, weapon) end

---Toggle Stinger ammunition on a supported recycled missile launcher.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.stinger(dbref, pilot, weapon) end

---Select an acquired friendly spotter; use own dbref to declare spotting, or nil to stop.
---@param dbref integer
---@param pilot integer
---@param spotter integer?
---@return boolean
function btech_unit.spot(dbref, pilot, spotter) end

---@class BattleHexLock
---@field hex {x: integer, y: integer}
---@field mode 'unit_at_hex'|'hex'|'building'|'ignite'|'clear'
---@field remaining integer Eight seconds to settle; zero is settled.

---Select valid map coordinates without requiring visibility. Unit-at-hex fire uses its current occupant; empty-hex and terrain attacks remain unimplemented.
---@param dbref integer
---@param pilot integer
---@param x integer
---@param y integer
---@param mode? string H/hex, B/building, I/ignite, C/clear; omitted means unit at hex.
---@return boolean
function btech_unit.lock_hex(dbref, pilot, x, y, mode) end

---@class BattleHexAimModifiers: BattleAimModifiers
---@field hex {x: integer, y: integer}
---@field mode 'unit_at_hex'|'hex'|'building'|'ignite'|'clear'
---@field visible boolean Current terrain visibility, separate from numeric aim.
---@field hex_bonus integer Zero for unit-at-hex, otherwise -4.
---@field subtotal integer|nil Nil beyond weapon range; numeric aim alone does not authorize firing.

---Inspect empty-terrain aim without dice, expenditure, or a fictitious target unit.
---@param dbref integer
---@param weapon integer
---@param x integer
---@param y integer
---@return BattleHexAimModifiers
function btech_unit.aim_hex(dbref, weapon, x, y) end

---Set a channel frequency without transmitting. Transactional; assigned conscious pilot required.
---@param dbref integer
---@param pilot integer
---@param channel integer Zero-based channel (A is 0).
---@param frequency integer From 0 through 999999.
---@return boolean
function btech_unit.radio_frequency(dbref, pilot, channel, frequency) end

---Save a title, truncated to fifteen bytes at a UTF-8 boundary. Transactional.
---@param dbref integer
---@param pilot integer
---@param channel integer Zero-based channel (A is 0).
---@param title string Empty clears the title.
---@return boolean
function btech_unit.radio_title(dbref, pilot, channel, title) end

---Replace channel mode: D digital, U muted, E relay; optional color letter. Transactional.
---Relay requires digital mode and capable hardware. Empty selects analog with no flags.
---@param dbref integer
---@param pilot integer
---@param channel integer Zero-based channel (A is 0).
---@param mode string
---@return boolean
function btech_unit.radio_mode(dbref, pilot, channel, mode) end

---@class BattleRadioReception
---@field receiver integer
---@field channel integer Zero-based receiving channel.
---@field transmitters integer[] Sender and any relays, excluding receiver.
---@field bearing integer Bearing toward final transmitter.
---@field text string Formatted cockpit message.

---@class BattleChannelMessage
---@field channel "debug"|"economy"|"attack_experience"|"experience"|"piloting_experience"|"frequencies"|"zero_frequencies"|"map_errors"
---@field text string

---@class BattleRadioTransmission
---@field delivery {mode: 'analog'|'digital', report: {sender: integer, map: integer, frequency: integer, receptions: BattleRadioReception[], interfered_receivers: integer[]?, scans: {receiver: integer, channel: integer, previous: integer, frequency: integer}[]?, notifications: BattleNotice[]?}}
---@field mines table Ordered frequency-matched command-mine report and consequences.
---@field audit_messages BattleChannelMessage[] Diagnostics committed with the transmission.
---@field experience_messages BattleChannelMessage[] Accepted communication XP diagnostics.

---Transmit using the selected channel; delivery and command mines commit together.
---Requires a conscious assigned cockpit pilot and no stun. Shutdown radios remain usable.
---@param dbref integer
---@param pilot integer
---@param channel integer Zero-based channel (A is 0).
---@param message string Nonempty text without control characters.
---@return BattleRadioTransmission
function btech_unit.radio_send(dbref, pilot, channel, message) end

---@class BattleTargetedRadioReport
---@field sender integer
---@field target integer
---@field notices {unit: integer, text: string}[] Captured sender echo and powered-recipient message.

---Send to an acquired visible target; the source must be running and not an observer.
---A shutdown target receives no message. This does not use channel frequencies or detonate mines.
---@param dbref integer
---@param pilot integer
---@param target integer Recipient dbref.
---@param message string Nonempty text without control characters.
---@return BattleTargetedRadioReport
function btech_unit.radio_target(dbref, pilot, target, message) end

---Inspect an acquired visible target without changing contacts or consuming dice.
---Stages a warning to running targets unless the scanning unit is an observer.
---Requires the conscious assigned pilot, a running unit and operational scanners.
---Observers bypass distance and receive exact status; ordinary scans disclose condition bands.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param target integer Target unit dbref.
---@param options string? A (armor), I (info), W (weapons), or a combination; omitted means all.
---@return string Styled scan report.
function btech_unit.scan(dbref, pilot, target, options) end

---Scan the first acquired visible occupant at a coordinate in saved map order.
---Uses the unit-scan report and warning path; empty and unacquired hexes share one reply.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param x integer Map column.
---@param y integer Map row.
---@param options string? A/I/W sections; omitted means all.
---@return string Styled report or empty-hex reply.
function btech_unit.scan_hex(dbref, pilot, x, y, options) end

---@class BattleBuildingScan
---@field text string Cockpit reply; undiscovered and missing buildings share one message.
---@field experience_messages BattleChannelMessage[] Accepted perception diagnostics.

---Scan a structure entrance and publish its integrity report to cockpit occupants.
---Hidden structures require an active in-character perception roll; invisible ones stay undetected.
---Dice, experience and output commit together. Explicit coordinates retain observer range limits.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param x integer Map column.
---@param y integer Map row.
---@return BattleBuildingScan
function btech_unit.scan_building(dbref, pilot, x, y) end

---@class BattleMineScan
---@field found boolean Successful recognition only; configuration is never disclosed.
---@field text string
---@field experience_messages BattleChannelMessage[]

---@class BattleHexScan
---@field building BattleBuildingScan
---@field mines BattleMineScan

---Scan buildings then mines in one transaction and publish both phases.
---Failed mine recognition is private to the pilot; success reaches cockpit occupants.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param x integer
---@param y integer
---@return BattleHexScan
function btech_unit.scan_terrain(dbref, pilot, x, y) end

---@class BattleSelectedScan
---@field kind 'unit'|'building'|'hex'
---@field report string|BattleBuildingScan|BattleHexScan

---Scan the saved unit or coordinate target without advancing its lock countdown.
---Unit reports are returned; building/hex reports also publish cockpit output.
---Selected coordinates allow observer distance exemptions while retaining visibility checks.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param options string? A/I/W unit report sections.
---@return BattleSelectedScan
function btech_unit.scan_selected(dbref, pilot, options) end

---Return a silent brief report of an acquired visible unit, with no armor or weapon details.
---Requires a conscious assigned pilot, running unit and working scanners. Direct reports
---do not impose the detailed scan radius. No dice, contacts or output are changed.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param target integer Target unit dbref.
---@return string Styled identity, motion and position summary.
function btech_unit.report(dbref, pilot, target) end

---@class BattleViewPosition
---@field map integer Scanner battlefield dbref.
---@field center {x: integer, y: integer} Requested center before viewport clipping.
---@field maximum_range integer Damage-adjusted display hardware radius.

---Resolve display centering only; this does not render or disclose terrain or occupants.
---Arguments are empty, a contact label/dbref, or integer bearing and signed distance.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param kind 'tactical'|'long_range'
---@param arguments string?
---@return BattleViewPosition
function btech_unit.view_center(dbref, pilot, kind, arguments) end

---@class BattleViewDimensions
---@field tactical_width integer? Requested columns, 5..40; default 21.
---@field tactical_height integer? Requested rows, 5..24; default 14.
---@field long_range_height integer? Requested rows, 10..40; default 11.

---@class BattleViewport
---@field map integer
---@field requested_center {x: integer, y: integer}
---@field origin {x: integer, y: integer} Upper-left in-bounds coordinate.
---@field width integer Column count.
---@field height integer Row count.
---@field maximum_range integer

---Resolve display bounds only, without rendering or disclosing terrain/occupants.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param kind 'tactical'|'long_range'
---@param arguments string? Own unit, target label/dbref, or bearing and distance.
---@param dimensions BattleViewDimensions?
---@return BattleViewport
function btech_unit.viewport(dbref, pilot, kind, arguments, dimensions) end

---@class BattleLongRangeMap
---@field viewport BattleViewport
---@field text string Filtered staggered-row display.

---Render long-range terrain, elevation or currently visible acquired units.
---Mode initials match native LRS; descriptive API mode names are also accepted.
---Dark maps mask unseen terrain. Rendering consumes no dice and sends no notices.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param mode string First letter T/E/C/M/L/H/S (case insensitive), or a descriptive API mode name.
---@param arguments string? Shared centering arguments.
---@return BattleLongRangeMap
function btech_unit.lrsmap(dbref, pilot, mode, arguments) end

---@class BattleTacticalMap
---@field viewport BattleViewport
---@field text string Styled hex display with acquired two-character contact labels.

---Render standard, C/T (mech/tank cliffs), B (landing zones), M (mines), L (visible), or U (underlying) tactical maps.
---Uses shared cockpit/display admission; no acquisition rolls, notices or state changes.
---@param dbref integer Scanner unit dbref.
---@param pilot integer
---@param arguments string? Optional C/T/B/M/L/U flag followed by shared centering arguments.
---@return BattleTacticalMap
function btech_unit.tactical(dbref, pilot, arguments) end

---@class BattleHexCenterReport
---@field coordinate BattleHexCoordinate
---@field elevation integer
---@field range number Horizontal range to the current hex center.
---@field bearing integer Clockwise degrees; 180 at the exact center.
---@field text string Shared native readout.

---Measure from continuous motion to the current hex center without scanner hardware.
---@param dbref integer
---@param pilot integer Conscious assigned pilot of a running unit.
---@return BattleHexCenterReport
function btech_unit.findcenter(dbref, pilot) end

---@class BattleNavigationReport
---@field center BattleHexCoordinate Requested local map center.
---@field text string Styled local map, continuous-position plot and live readouts.

---Show the radius-two local map and units within the selected center hex.
---@param dbref integer
---@param pilot integer Conscious assigned pilot of a running unit.
---@param arguments string? Own unit, contact label/dbref, or bearing and distance.
---@return BattleNavigationReport
function btech_unit.navigate(dbref, pilot, arguments) end

---Read saved map dimensions or replace them; omitted fields in a replacement use standard defaults.
---Trusted callback code owns authorization to change the selected player's preferences.
---Native tactical/LRS and Lua tactical/LRS/viewport use these defaults; navigate stays radius two.
---@param player integer Live player dbref.
---@param dimensions BattleViewDimensions? Validated replacement; omit for a read-only query.
---@return BattleViewDimensions
function btech_player.view_dimensions(player, dimensions) end

---@alias BattleBuildingContactMode "follow_brief" | "include" | "exclude"

---@class BattleContactPreferences
---@field include_dead boolean Defaults false.
---@field include_shutdown boolean Defaults true.
---@field include_enemies boolean Defaults true.
---@field include_allies boolean Defaults true.
---@field include_target boolean Defaults true; never bypasses visibility.
---@field buildings BattleBuildingContactMode Defaults "exclude"; applies to native contacts +, independently of unit filtering.

---Read or replace saved contact-list inclusion policy for a live player.
---Trusted callback code owns edit authorization; omitted replacement fields use defaults.
---@param player integer
---@param preferences BattleContactPreferences?
---@return BattleContactPreferences
function btech_player.contact_preferences(player, preferences) end

---@class BattleContactOptions
---@field buildings boolean Include building contacts for native output.
---@field preferences BattleContactPreferences Decoded unit categories.
---@field ignored string[] Unrecognized characters in encounter order.

---Decode transient unit-list options d/s/e/a/t and persistent exclusion prefix !.
---Does not access game state; b requests building identification in native output.
---@param options string Single word, up to fifty characters are processed.
---@param brief_buildings boolean? Initial building inclusion from unit brief settings; defaults false.
---@return BattleContactOptions
function btech_player.contact_options(options, brief_buildings) end

---@class BattleBuildingContact
---@field detection BattleDetectionChannel|nil Whether the sensor band or sight reaches the entrance.
---@field short_text string Plain compact row after identification locks.
---@field weapon_arc BattleContactArc Observer torso direction toward entrance.
---@field interior integer
---@field coordinate BattleHexCoordinate
---@field elevation integer
---@field name string Plain structure name.
---@field range number
---@field bearing integer
---@field integrity integer
---@field maximum_integrity integer
---@field identified boolean Identification lock result.
---@field hidden boolean Concealed entrance identified successfully.
---@field status string Blank, x (restricted), X (safe/restricted command center), or C (command center).

---List visible structures using silent identify_building locks. Failed evaluations roll back side effects.
---@param unit integer
---@param pilot integer Conscious assigned pilot of a running unit.
---@return BattleBuildingContact[]
function btech_unit.building_contacts(unit, pilot) end

---@class BattleBriefSettings
---@field contacts integer Contact mode 0..3; defaults 1.
---@field automatic integer Routine notice mode 0..6; defaults 0.

---@class BattleBriefReport
---@field settings BattleBriefSettings
---@field changed boolean An edit was requested; query is false.
---@field text string Query or cockpit confirmation.

---Query unit display settings or edit A/C independently. Edits notify occupants.
---Requires conscious cockpit occupant; shutdown is allowed. Errors roll back state and notices.
---@param unit integer
---@param pilot integer
---@param arguments string? Empty for query, A 0..6 or C 0..3 for edits.
---@return BattleBriefReport
function btech_unit.brief(unit, pilot, arguments) end

---Set whether routine contact notices include shutdown targets. Acquisition is unchanged.
---@param dbref integer
---@param player integer Assigned cockpit pilot.
---@param enabled boolean
---@return boolean
function btech_unit.autocon_shutdown(dbref, player, enabled) end

---Request a six-second lateral change; requires an intact quad and its assigned pilot.
---@param dbref integer
---@param player integer
---@param direction string nw/fl, ne/fr, sw/rl, se/rr, or - to travel straight.
---@return BattleNotice
function btech_unit.lateral(dbref, player, direction) end

---@class BattleBootleggerReport
---@field modifier integer Situational difficulty and failed-fall severity.
---@field check BattlePilotingCheck
---@field fall BattleFallReport?
---@field notices BattleNotice[]

---Pivot left/right on a piloting check; a failed attempt falls.
---@param dbref integer
---@param player integer
---@param direction string
---@return BattleBootleggerReport
function btech_unit.bootlegger(dbref, player, direction) end

---@class BattleEtaReport
---@field coordinate BattleHexCoordinate
---@field range number Horizontal range.
---@field minutes integer? Whole minutes, absent when effectively stationary.
---@field text string

---Estimate travel to explicit x y or the selected ordinary hex and notify cockpit occupants.
---@param dbref integer
---@param player integer
---@param coordinates string?
---@return BattleEtaReport
function btech_unit.eta(dbref, player, coordinates) end

---@class BattleBearingReport
---@field origin BattlePoint
---@field destination BattlePoint
---@field bearing integer Clockwise compass degrees, 180 for coincident points.
---@field text string

---Read a compass bearing to the default target, x y, or x0 y0 x1 y1.
---@param dbref integer
---@param player integer
---@param coordinates string?
---@return BattleBearingReport
function btech_unit.bearing(dbref, player, coordinates) end

---@class BattleRangeReport
---@field horizontal number Horizontal distance in hexes.
---@field spatial number Spatial distance after dark-map terrain masking.
---@field text string

---Read range to default target, x y, or between x0 y0 and x1 y1.
---@param dbref integer
---@param player integer
---@param coordinates string?
---@return BattleRangeReport
function btech_unit.range_report(dbref, player, coordinates) end

---@class BattleVectorReport
---@field horizontal number
---@field spatial number
---@field bearing integer Clockwise compass bearing.
---@field vertical_bearing integer Signed vertical angle rounded away from zero.
---@field text string

---Measure a default target, destination x/y[/z], or origin and destination x/y[/z].
---@param dbref integer
---@param player integer
---@param coordinates string?
---@return BattleVectorReport
function btech_unit.vector(dbref, player, coordinates) end

---Start or stop ammunition dumping; weapon numbers are zero based and slots one based.
---@param dbref integer
---@param player integer
---@param selection string
---@return table[] notices
function btech_unit.dump(dbref, player, selection) end

---@class BattleBoosterState
---@field enabled boolean
---@field counter integer
---@field remaining integer Seconds until the next overload/recovery check.
---@field failed boolean Hardware failure persists through shutdown.

---Toggle MASC and adjust the desired throttle proportionally.
---@param dbref integer
---@param pilot integer
---@return table notice
function btech_unit.masc(dbref, pilot) end

---Toggle the supercharger and adjust the desired throttle proportionally.
---@param dbref integer
---@param pilot integer
---@return table notice
function btech_unit.supercharger(dbref, pilot) end

---Join a visible friendly unit's C3i network by battlefield ID, or leave with "-".
---@param dbref integer
---@param pilot integer
---@param target string
---@return BattleNotice[]|nil
---@return table|nil error
function btech_unit.c3i(dbref, pilot, target) end

---Send text to available C3i peers and echo it to your cockpit. Requires an active transaction.
---@param dbref integer
---@param pilot integer
---@param message string
---@return BattleNotice[]|nil
---@return table|nil error
function btech_unit.c3i_message(dbref, pilot, message) end

---@class BattleNetworkStatusRow
---@field unit integer
---@field label string
---@field name string
---@field coordinate BattleHexCoordinate
---@field elevation integer
---@field range number
---@field bearing integer
---@field speed number
---@field heading integer
---@field armor_percent integer
---@field internal_percent integer

---Inspect running, unjammed peers without requiring visual contact or publishing output.
---@param dbref integer
---@param pilot integer
---@return {rows: BattleNetworkStatusRow[], text: string}|nil
---@return table|nil error
function btech_unit.c3i_network(dbref, pilot) end

---@class BattleNetworkTargetRow
---@field unit integer
---@field label string
---@field name string
---@field identified boolean
---@field friendly boolean Actual team relationship; identification controls display color.
---@field detection BattleDetectionChannel|nil How the requester itself perceives the target; nil for network-only sightings.
---@field weapon_arc string
---@field coordinate BattleHexCoordinate
---@field elevation integer
---@field range number Physical spatial range.
---@field network_range {kind: "c3"|"c3i", distance: number, source: integer|nil}
---@field bearing integer
---@field speed number
---@field heading integer
---@field status string Five condition columns; blank without a clear sighting.
---@field destroyed boolean
---@field selected boolean

---Inspect direct and network sightings without acquiring contacts or publishing output.
---@param dbref integer
---@param pilot integer
---@return {rows: BattleNetworkTargetRow[], text: string}|nil
---@return table|nil error
function btech_unit.c3i_targets(dbref, pilot) end

---Join a visible friendly classic C3 network, or leave with "-". Capacity depends on working masters.
---@param dbref integer
---@param pilot integer
---@param target string
---@return BattleNotice[]|nil
---@return table|nil error
function btech_unit.c3(dbref, pilot, target) end

---Send to available classic C3 peers using current master capacity; echo to your cockpit.
---@param dbref integer
---@param pilot integer
---@param message string
---@return BattleNotice[]|nil
---@return table|nil error
function btech_unit.c3_message(dbref, pilot, message) end

---Inspect classic C3 peers using active master capacity; emits no messages.
---@param dbref integer
---@param pilot integer
---@return {rows: BattleNetworkStatusRow[], text: string}|nil
---@return table|nil error
function btech_unit.c3_network(dbref, pilot) end

---Inspect direct and classic C3 target sightings without acquiring contacts.
---@param dbref integer
---@param pilot integer
---@return {rows: BattleNetworkTargetRow[], text: string}|nil
---@return table|nil error
function btech_unit.c3_targets(dbref, pilot) end

---Inspect or set a running vehicle turret's absolute heading. Set accepts integer degrees. Transactional.
---@param dbref integer
---@param pilot integer
---@param heading integer|nil
---@return number|boolean
function btech_unit.turret(dbref, pilot, heading) end

--- Begin a 60-second turret repair, blocking fire until pending attempts finish.
---@param unit integer
---@param pilot integer
---@return boolean
function btech_unit.fixturret(unit, pilot) end

---Begin a two-minute attempt to put out vehicle section fires while shut down.
---@param dbref integer
---@param pilot integer
---@return boolean
function btech_unit.extinguish(dbref, pilot) end

---Pick up a visible unit using shared towing, shutdown and terrain rules.
---@param dbref integer Carrier unit.
---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
---@param target integer Target unit.
---@return boolean
function btech_unit.pickup(dbref, pilot, target) end

---Release the carrier's tow; elevated targets begin forced descent.
---@param dbref integer Carrier unit.
---@param pilot integer Conscious assigned pilot.
---@return boolean
function btech_unit.dropoff(dbref, pilot) end

---Inspect or set scenario permission to tow this unit out of character.
---Trusted scripts own authorization for edits; this is not a pilot preference.
---Disabling permission does not release an existing tow.
---@param dbref integer
---@param enabled boolean? Omit to inspect without changing state.
---@return boolean Current permission.
function btech_unit.towable(dbref, enabled) end

---Begin twenty seconds of digging in a stopped tracked or wheeled vehicle.
---Completed cover permits only turret weapons; requesting movement leaves cover.
---@param dbref integer
---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
---@return boolean
function btech_unit.dig(dbref, pilot) end

---Lower a quad, raise it with "-", or cancel its pending change with "stop".
---@param dbref integer
---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
---@param argument string? Omit to lower.
---@return boolean
function btech_unit.hulldown(dbref, pilot, argument) end

---Inspect or set scenario fortification. Trusted scripts own authorization.
---Enabling requires settled motion, no tow relationship, no building-entry request,
---and a landed unit. Disabling does not restart any action.
---@param dbref integer
---@param enabled boolean? Omit to inspect.
---@return boolean Current fortification state.
function btech_unit.fortified(dbref, enabled) end

---Inspect or set observer role. Trusted scripts own authorization; cockpit pilots cannot grant this role.
---@param dbref integer
---@param enabled boolean? Omit to inspect the saved role.
---@return boolean
function btech_unit.observer(dbref, enabled) end

---Read or change operator weapons hold inside a trusted callback transaction.
---Hold blocks fire and TIC admission before argument decoding or loss of cover.
---The setting persists through shutdown and restart; aborted callbacks restore it.
---@param dbref integer Constructed unit dbref.
---@param enabled boolean|nil Omit to inspect without changing the setting.
---@return boolean enabled
function btech_unit.weapons_hold(dbref, enabled) end

---Detonate a Mech reactor in a trusted callback; damage, sensor flashes and casualties commit together.
---This scenario action bypasses cockpit self-destruct configuration and countdown admission.
---@param dbref integer
---@return table report
function btech_unit.reactor_explode(dbref) end

---Start or stop cockpit self-destruction. Engagement releases the pilot assignment.
---The same argument grammar, configuration and override checks apply as the native explode command.
---@param dbref integer
---@param pilot integer
---@param argument string "ammo", "reactor", "stop", optionally followed by wizard "override".
---@return boolean
function btech_unit.explode(dbref, pilot, argument) end

---Set scenario protection from new ammunition self-destruct requests; admitted timers continue.
---@param dbref integer
---@param enabled boolean
---@return boolean
function btech_unit.explode_safe(dbref, enabled) end

---Read or replace trusted scenario visibility. Both fields are required when replacing it.
---Clairvoyance bypasses visibility checks; ordinary sensor acquisition still rejects invisible targets.
---@param dbref integer
---@param flags {invisible: boolean, clairvoyant: boolean}?
---@return {invisible: boolean, clairvoyant: boolean}
function btech_unit.visibility(dbref, flags) end

---Read or change scenario combat immunity in a trusted callback transaction.
---@param dbref integer
---@param enabled boolean|nil Omit to inspect.
---@return boolean
function btech_unit.combat_safe(dbref, enabled) end

---Select Swarm missiles; unused missiles can retarget friendly units, including the launcher.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.fireswarm(dbref, pilot, weapon) end

---Select Swarm-1 missiles; secondary targets must belong to another team.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.fireswarm1(dbref, pilot, weapon) end

---@class BattleSwarmHop
---@field target integer
---@field incoming integer
---@field roll integer
---@field remaining integer
---@field salvo {kind: 'mech'|'vehicle', report: table}|nil Absent for a secondary miss.

---@class BattleSwarmReport
---@field launched integer
---@field remaining integer
---@field traveled number Cumulative distance, including a terminal leg that falls short.
---@field hops BattleSwarmHop[] Ordered attacks, at most eleven.
---@field notices BattleNotice[]
---@field broadcasts BattleNotice[]

---Alias of cluster: select artillery cluster rounds, rejecting a different selected artillery payload.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.firecluster(dbref, pilot, weapon) end

---Select missile Smoke rounds. This cockpit control does not select artillery payloads.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.firesmoke(dbref, pilot, weapon) end

---Select missile Mine rounds, which bypass AMS and retain ordinary missile damage.
---@param dbref integer
---@param pilot integer
---@param weapon integer
---@return BattleAmmunitionMode
function btech_unit.firemine(dbref, pilot, weapon) end

---@class BattleWeaponDamage
---@field location {section: BattleSectionName, slot: integer}
---@field effects ("moderate"|"focus"|"crystal"|"ranging"|"barrel"|"feed")[] Distinct component effects; empty means superficial damage.

---Check map membership and world invariants without changing placements or unit state.
---@param actor integer Wizard performing the check.
---@param map integer
---@return table report Map ID and units in persisted slot order.
function btech_map.check(actor, map) end

---Publish and return a wizard's map field report in catalogue order.
---@param actor integer
---@param map integer
---@param arguments? string Optional leading 1 or 4 selects columns, followed by a field prefix.
---@return table report Map, columns, fields (name/value) and literal text. firstfree has no value.
function btech_map.fields(actor, map, arguments) end

---Publish a wizard-only skill leaderboard without changing XP.
---@param actor integer
---@param skill string Canonical skill name or alias.
---@return table report Skill, counted players, total balance, top sixteen entries and literal text.
function btech_character.xptop(actor, skill) end

---Request persistence of the current world at transaction commit, even if unchanged.
---@param actor integer Wizard requesting the checkpoint.
---@return boolean queued Success is published only after persistence; rollback cancels the request.
function btech_database.save(actor) end

---Return all part/manufacturer forms in short-name order, without requiring live stock.
---@param actor integer Wizard requesting inspection.
---@return table[] forms Part ID, brand ID, short_name, long_name and very_long_name.
function btech_inventory.forms(actor) end

---@class BattleRuntimeStats
---@field simulation_pending boolean Same work predicate as the server's one-second simulation tick.
---@field scanner_observers integer
---@field reactor_startup_remaining integer
---@field artillery_shots integer
---@field maps integer
---@field mechs integer
---@field vehicles integer
---@field registration_kinds table<string, integer>
---@field inline_record_bytes integer Root/map/unit inline sizes only; heap storage excluded.
---@field encoded_state_bytes integer Exact compact JSON encoding size, not allocator usage.

---Wizard-only detached snapshot; does not advance simulation or consume dice.
---@param actor integer
---@return BattleRuntimeStats
function btech_runtime.stats(actor) end

---Wizard-only predictive firing using fixed horizontal target orders and normal weapon launches.
---Sets the cockpit hex target. Does not simulate future damage or order changes.
---@param dbref integer Shooter unit
---@param player integer Assigned wizard pilot
---@param target integer Target unit on the same battlefield
---@param selection string Comma-separated weapon numbers and inclusive ranges
---@return boolean success
function btech_unit.snipe(dbref, player, target, selection) end

---@class BattleUnitField
---@field name string Full field name, independent of display width.
---@field value string|nil Available field value; nil displays as n/a.

---@class BattleUnitFieldReport
---@field unit integer
---@field columns integer
---@field fields BattleUnitField[]
---@field text string Literal report text, already published to the actor.

---Wizard field inspection using optional 1/4 column selector and case-insensitive prefix.
---@param actor integer
---@param unit integer
---@param arguments? string
---@return BattleUnitFieldReport
function btech_unit.fields(actor, unit, arguments) end

---Wizard named edits; accepts identity, team, xpmod, VTOL fuel, sensor/radio hardware and Mech thermal fields.
---@param actor integer
---@param unit integer
---@param field string
---@param value string
function btech_unit.set_field(actor, unit, field, value) end

-- C-parity contract surface shared by several groups below.

---Typed unit-layout section constant from btech.unit.sections.
---@class BattleSection
---Typed unit class constant from btech.unit.types.
---@class BattleUnitType
---Typed movement class constant from btech.unit.movement_types.
---@class BattleMovementType
---Typed technology code from btech.unit.technology.
---@class BattleTechnologyCode
---Typed technology group from btech.unit.technology_groups.
---@class BattleTechnologyGroup
---Typed weapon fire-mode constant from btech.unit.fire_modes.
---@class BattleFireModeConstant
---Typed ammunition-mode constant from btech.unit.ammunition_modes.
---@class BattleAmmunitionModeConstant
---Typed repair operation from btech.repair.operations.
---@class BattleRepairOperation
---Typed battlefield light constant from btech.map.light_levels.
---@class BattleLightLevel

---@class BattleValuePair
---@field current integer
---@field original integer

---@class BattleArmorStatus
---@field section? BattleSection Omitted when the request did not select one.
---@field armor BattleValuePair
---@field internal BattleValuePair
---@field rear_armor BattleValuePair

---@class BattleAmmunitionStatus
---@field rounds integer
---@field capacity integer

---@class BattleWeaponStats
---@field kind string
---@field heat integer
---@field damage integer
---@field minimum_range integer
---@field short_range integer
---@field medium_range integer
---@field long_range integer
---@field critical_slots integer
---@field ammunition_per_ton integer
---@field recycle_time integer
---@field battle_value integer

---@class BattlePartDefinition
---@field id integer Stable catalogue part identifier.
---@field brand integer Manufacturer identifier.
---@field packed_id integer Brand-major combined identifier.
---@field short_name string
---@field long_name string
---@field very_long_name string
---@field category string
---@field weight_tons number
---@field cost integer
---@field weapon? BattleWeaponStats Present for weapon parts.

---@alias BattlePartRef BattlePartDefinition|integer|string

---@class BattlePartStack
---@field part BattlePartDefinition
---@field quantity integer

---@class BattlePartCategory
---@field code string
---@field name string

---@class BattleCriticalSlot
---@field section BattleSection
---@field slot integer
---@field kind string
---@field part? BattlePartDefinition
---@field operational boolean
---@field temporary_failure boolean
---@field auxiliary_data integer
---@field ammunition? BattleAmmunitionStatus
---@field fire_modes BattleFireModeConstant[]
---@field ammunition_modes BattleAmmunitionModeConstant[]

---@class BattleMountedWeapon
---@field number integer Zero-based stable weapon number.
---@field section BattleSection
---@field first_slot integer Zero-based first occupied critical slot.
---@field part BattlePartDefinition
---@field slot_count integer
---@field recycle integer Seconds remaining in the current cycle.
---@field recycle_time integer Full recycle time in seconds.
---@field operational boolean

---@class BattleEngine
---@field rating integer
---@field suspension_factor integer

---@class BattleRadioChannelReport
---@field channel integer One-based channel position.
---@field frequency integer Frequency from 0 through 999999.
---@field title string At most fifteen UTF-8 bytes.
---@field modes string[] Active mode names: digital, mute, relay, information, scan.

---@class BattleBattleValue
---@field total number
---@field offensive number
---@field defensive number

---@class BattleTechnology
---@field code BattleTechnologyCode
---@field name string
---@field group "primary"|"secondary"|"infantry"
---@field source "configured"|"inferred"

-- C-parity character value and progress contracts.

---@class BattleCharacterValueDefinition
---@field code integer
---@field name string
---@field kind string Char_value, Char_skill, Char_advantage or Char_attribute.
---@field default_experience_threshold integer

---@class BattleCharacterValueReport
---@field definition BattleCharacterValueDefinition
---@field amount integer
---@field target? integer Skill targets including earned levels.
---@field experience? integer
---@field experience_to_next_level? integer

---Add signed skill experience using the shared C range semantics.
---@param character DbRef|Object Player object.
---@param skill string Canonical name or alias.
---@param amount integer
function btech_character.add_skill_experience(character, skill, amount) end

---Return ordered value definitions of one kind; a supplied player filters unsaved
---skills and advantages while attributes stay complete.
---@param kind string Char_value, Char_skill, Char_advantage or Char_attribute.
---@param character? DbRef|Object
---@return BattleCharacterValueDefinition[] definitions
function btech_character.catalog(kind, character) end

---Read the configured runtime experience threshold of one skill.
---@param skill string Canonical name or alias.
---@return integer threshold
function btech_character.experience_threshold(skill) end

---Replace the stored unsigned 32-bit skill experience.
---@param character DbRef|Object Player object.
---@param skill string
---@param experience integer
function btech_character.set_skill_experience(character, skill, experience) end

---Set the raw skill amount needed for the requested target; rejects non-skills and
---unreachable targets.
---@param character DbRef|Object Player object.
---@param skill string
---@param target integer
function btech_character.set_skill_target(character, skill, target) end

---Set one character value by name or code, preserving the C unsigned-byte storage.
---@param character DbRef|Object Player object.
---@param value string|integer Character-value name or code.
---@param amount integer
function btech_character.set_value(character, value, amount) end

---Read one character value; skills additionally report target and experience progress.
---@param character DbRef|Object Player object.
---@param value string|integer Character-value name, prefix or code.
---@return BattleCharacterValueReport result
function btech_character.value(character, value) end

-- C-parity personal-combat and player-preference contracts.

---@class BattlePersonalCombatArmor
---@field head integer
---@field torso integer
---@field hands integer
---@field feet integer

---@class BattlePersonalCombatEquipment
---@field weapon BattlePartDefinition
---@field ammunition? integer

---@class BattlePersonalCombatLoadout
---@field armor BattlePersonalCombatArmor
---@field right? BattlePersonalCombatEquipment
---@field left? BattlePersonalCombatEquipment

---@class BattleUiPreferencesState
---@field tactical_height integer
---@field tactical_width integer
---@field lrs_height integer
---@field include_dead boolean
---@field include_shutdown boolean
---@field include_enemies boolean
---@field include_allies boolean
---@field include_target boolean
---@field buildings "follow_brief"|"include"|"exclude"
---@field configured boolean

---Read the saved personal-combat loadout, or nil when none is configured.
---@param player DbRef|Object
---@return BattlePersonalCombatLoadout|nil loadout
function btech_player.loadout(player) end

---Replace the saved personal-combat loadout; nil clears it.
---@param player DbRef|Object
---@param loadout BattlePersonalCombatLoadout|nil
function btech_player.set_loadout(player, loadout) end

---Read the saved MechWarrior template reference, or nil when unset.
---@param player DbRef|Object
---@return string|nil reference
function btech_player.mechwarrior_template(player) end

---Replace the saved MechWarrior template reference; nil clears it.
---@param player DbRef|Object
---@param reference string|nil
function btech_player.set_mechwarrior_template(player, reference) end

---Read the saved tactical contact and display preferences.
---@param player DbRef|Object
---@return BattleUiPreferencesState preferences
function btech_player.ui_preferences(player) end

---Replace the saved tactical contact and display preferences; nil clears them.
---@param player DbRef|Object
---@param preferences BattleUiPreferencesState|nil
function btech_player.set_ui_preferences(player, preferences) end

-- C-parity template inspection contracts.

---Read current, original and rear armor values; an omitted section reports the totals.
---@param reference string Relative name under database.mech_database.
---@param section? BattleSection Typed section constant from btech.unit.sections.
---@return BattleArmorStatus status
function btech_template.armor(reference, section) end

---Read the constructed base cost in C-bills.
---@param reference string
---@return integer cost
function btech_template.base_cost(reference) end

---Read offensive, defensive and total Battle Value.
---@param reference string
---@return BattleBattleValue value
function btech_template.battle_value(reference) end

---List one section's critical slots with resolved parts, modes and ammunition state.
---@param reference string
---@param section BattleSection Typed section constant from btech.unit.sections.
---@return BattleCriticalSlot[] slots
function btech_template.critical_slots(reference, section) end

---Read the engine rating and suspension factor.
---@param reference string
---@return BattleEngine engine
function btech_template.engine(reference) end

---Report whether the reference resolves to a loadable template.
---@param reference string
---@return boolean exists
function btech_template.exists(reference) end

---List installed equipment in catalogue order.
---@param reference string
---@return BattlePartStack[] parts
function btech_template.installed_parts(reference) end

---List carried ammunition stock in catalogue order.
---@param reference string
---@return BattlePartStack[] parts
function btech_template.payload(reference) end

---Publish the full template status report to a player.
---@param reference string
---@param player DbRef|Object
function btech_template.show_status(reference, player) end

---Publish the template weapon specifications to a player.
---@param reference string
---@param player DbRef|Object
function btech_template.show_weapon_specs(reference, player) end

---Publish one section's critical status report to a player.
---@param reference string
---@param player DbRef|Object
---@param section BattleSection Typed section constant from btech.unit.sections.
function btech_template.show_critical_status(reference, player, section) end

---List configured and inferred technologies.
---@param reference string
---@return BattleTechnology[] technologies
function btech_template.technologies(reference) end

---List mounted weapons in mounting order; an optional section restricts the result.
---@param reference string
---@param section? BattleSection Typed section constant from btech.unit.sections.
---@return BattleMountedWeapon[] weapons
function btech_template.weapons(reference, section) end

-- C-parity unit inspection and administration contracts.

---Install one technology code on the unit.
---@param unit DbRef|Object
---@param technology BattleTechnologyCode Typed constant from btech.unit.technology.
function btech_unit.add_technology(unit, technology) end

---Remove one installed technology code.
---@param unit DbRef|Object
---@param technology BattleTechnologyCode Typed constant from btech.unit.technology.
function btech_unit.remove_technology(unit, technology) end

---Remove every technology in one group.
---@param unit DbRef|Object
---@param group BattleTechnologyGroup Typed constant from btech.unit.technology_groups.
function btech_unit.clear_technologies(unit, group) end

---List configured and inferred unit technologies.
---@param unit DbRef|Object
---@return BattleTechnology[] technologies
function btech_unit.technologies(unit) end

---Apply a C-contract damage request to a live unit.
---@param unit DbRef|Object
---@param request table Damage request record.
function btech_unit.apply_damage(unit, request) end

---Read current, original and rear armor values; an omitted section reports the totals.
---@param unit DbRef|Object
---@param section? BattleSection Typed section constant from btech.unit.sections.
---@return BattleArmorStatus status
function btech_unit.armor(unit, section) end

---Read the assigned pilot object, or nil when the cockpit is unassigned.
---@param unit DbRef|Object
---@return Object|nil pilot
function btech_unit.assigned_pilot(unit) end

---Read offensive, defensive and total Battle Value.
---@param unit DbRef|Object
---@return BattleBattleValue value
function btech_unit.battle_value(unit) end

---List one section's critical slots with resolved parts, modes and ammunition state.
---@param unit DbRef|Object
---@param section BattleSection Typed section constant from btech.unit.sections.
---@return BattleCriticalSlot[] slots
function btech_unit.critical_slots(unit, section) end

---Read the engine rating and suspension factor.
---@param unit DbRef|Object
---@return BattleEngine engine
function btech_unit.engine(unit) end

---List installed equipment in catalogue order.
---@param unit DbRef|Object
---@return BattlePartStack[] parts
function btech_unit.installed_parts(unit) end

---List carried ammunition stock in catalogue order.
---@param unit DbRef|Object
---@return BattlePartStack[] parts
function btech_unit.payload(unit) end

---Replace the unit definition from a saved template reference.
---@param unit DbRef|Object
---@param reference string Relative name under database.mech_database.
function btech_unit.load_template(unit, reference) end

---Save the unit definition under a template reference in the mech database.
---@param unit DbRef|Object
---@param reference string Relative name under database.mech_database.
function btech_unit.save_template(unit, reference) end

---Run one shared piloting check; returns whether it succeeded.
---@param unit DbRef|Object
---@param options table Situational modifier request.
---@return boolean succeeded
function btech_unit.piloting_check(unit, options) end

---Read the saved two-letter battlefield ID preference, or nil when unset.
---@param unit DbRef|Object
---@return string|nil id
function btech_unit.preferred_id(unit) end

---List configured radio channels with active mode names.
---@param unit DbRef|Object
---@return BattleRadioChannelReport[] channels
function btech_unit.radio_channels(unit) end

---Restore destroyed critical slots to their original equipment.
---@param unit DbRef|Object
function btech_unit.reset_critical_slots(unit) end

---Refill one ammunition bin to its installed capacity.
---@param unit DbRef|Object
---@param section BattleSection Typed section constant from btech.unit.sections.
---@param slot integer One-based critical slot.
function btech_unit.restock_ammunition(unit, section, slot) end

---Restore armor, internal structure, critical slots and ammunition to template values.
---@param unit DbRef|Object
function btech_unit.restore(unit) end

---Read a section's damage condition.
---@param unit DbRef|Object
---@param section BattleSection Typed section constant from btech.unit.sections.
---@return "operational"|"destroyed"|"flooded" condition
function btech_unit.section_condition(unit, section) end

---@class BattleWeaponInstall
---@field part BattlePartRef Weapon part reference.
---@field section BattleSection
---@field slots integer[] Zero-based critical slots.
---@field rear_facing? boolean
---@field targeting_computer? boolean
---@field one_shot? boolean

---@class BattleAmmunitionConfiguration
---@field weapon BattlePartRef Launcher part reference.
---@field section BattleSection
---@field slot integer Zero-based critical slot.
---@field half_ton? boolean
---@field ammunition_modes? BattleAmmunitionModeConstant[]

---@class BattleWeaponModes
---@field fire_modes? BattleFireModeConstant[]
---@field ammunition_modes? BattleAmmunitionModeConstant[]

---@class BattleSpecialInstall
---@field part? BattlePartRef Omit to empty the slot.
---@field section BattleSection
---@field slot integer Zero-based critical slot.
---@field auxiliary_data? integer

---Install a weapon into explicit critical slots.
---@param unit DbRef|Object
---@param request BattleWeaponInstall
function btech_unit.install_weapon(unit, request) end

---Install or clear non-weapon equipment in one critical slot.
---@param unit DbRef|Object
---@param request BattleSpecialInstall
function btech_unit.install_special(unit, request) end

---Configure one ammunition bin's half-ton flag and selected modes.
---@param unit DbRef|Object
---@param request BattleAmmunitionConfiguration
function btech_unit.configure_ammunition(unit, request) end

---Replace the selected fire and ammunition modes of one mounted weapon.
---@param unit DbRef|Object
---@param weapon_number integer Zero-based stable weapon number.
---@param modes BattleWeaponModes
function btech_unit.set_weapon_modes(unit, weapon_number, modes) end

---Patch armor values on one section.
---@param unit DbRef|Object
---@param section BattleSection Typed section constant from btech.unit.sections.
---@param patch table Current-armor, internal or rear-armor integers, each 0 through 255.
function btech_unit.set_armor(unit, section, patch) end

---Assign or clear the saved pilot; the player need not enter the cockpit.
---@param unit DbRef|Object
---@param pilot DbRef|Object|nil Player object.
function btech_unit.set_assigned_pilot(unit, pilot) end

---Set cargo space and the maximum carried tonnage.
---@param unit DbRef|Object
---@param space integer
---@param maximum_tons integer
function btech_unit.set_cargo_capacity(unit, space, maximum_tons) end

---Set the installed heat-sink count.
---@param unit DbRef|Object
---@param count integer
function btech_unit.set_heat_sinks(unit, count) end

---Set the jump speed in movement points.
---@param unit DbRef|Object
---@param movement_points number
function btech_unit.set_jump_speed(unit, movement_points) end

---Set the long-range sensor ceiling in hexes.
---@param unit DbRef|Object
---@param range integer
function btech_unit.set_long_range_sensor_range(unit, range) end

---Set the tactical sensor range in hexes.
---@param unit DbRef|Object
---@param range integer
function btech_unit.set_tactical_range(unit, range) end

---Set the scan range in hexes.
---@param unit DbRef|Object
---@param range integer
function btech_unit.set_scan_range(unit, range) end

---Set the radio range in hexes.
---@param unit DbRef|Object
---@param range integer
function btech_unit.set_radio_range(unit, range) end

---Set the maximum ground speed in movement points.
---@param unit DbRef|Object
---@param movement_points number
function btech_unit.set_max_speed(unit, movement_points) end

---Replace the movement class.
---@param unit DbRef|Object
---@param movement_type BattleMovementType Typed constant from btech.unit.movement_types.
function btech_unit.set_movement_type(unit, movement_type) end

---Set the unit tonnage.
---@param unit DbRef|Object
---@param tons integer
function btech_unit.set_tonnage(unit, tons) end

---Replace the unit class.
---@param unit DbRef|Object
---@param unit_type BattleUnitType Typed constant from btech.unit.types.
function btech_unit.set_unit_type(unit, unit_type) end

---Set the radio quality grade.
---@param unit DbRef|Object
---@param quality integer
function btech_unit.set_radio_quality(unit, quality) end

---Remove the object's BattleTech registration and forget its configuration
---references. Rust extension without a C Lua counterpart: the reference exposes
---teardown only through the native wizard command, and this binding shares that
---command's teardown exactly. Succeeds silently for an already-plain object and
---never moves or destroys the container thing; mutations join the surrounding
---callback transaction.
---@param unit DbRef|Object Live thing to tear down.
---@return boolean true
function btech_unit.unregister(unit) end

---Read the damage-adjusted maximum speed in movement points.
---@param unit DbRef|Object
---@return number movement_points
function btech_unit.effective_max_speed(unit) end

---Read the damage-adjusted maximum speed in kilometers per hour.
---@param unit DbRef|Object
---@return number kilometers_per_hour
function btech_unit.effective_max_speed_kph(unit) end

---List the weapons of one trigger group in mounting order.
---@param unit DbRef|Object
---@param tic integer Group number from 0 through 3.
---@return BattleMountedWeapon[] weapons
function btech_unit.tic_weapons(unit, tic) end

-- C-parity part catalogue contracts.

local btech_parts = {}

---Apply one signed atomic stock edit; a nonzero integral delta is required.
---@param target DbRef|Object Live object holding stock.
---@param part BattlePartRef
---@param delta integer
function btech_parts.adjust_stores(target, part, delta) end

---Return the six detached part categories in canonical order.
---@return BattlePartCategory[] categories
function btech_parts.categories() end

---List registered branded forms in catalogue order; a case-insensitive category filters them.
---@param category? string
---@return BattlePartDefinition[] parts
function btech_parts.list(category) end

---Resolve one registered part by packed ID, case-insensitive name or {id, brand} record.
---@param part BattlePartRef
---@return BattlePartDefinition|nil part
function btech_parts.resolve(part) end

---Search names with *, ? and backslash-escaped quick-wild matching.
---@param query string Nonempty query.
---@return BattlePartDefinition[] parts
function btech_parts.search(query) end

---Set the cost shared by every brand of one registered part.
---@param part BattlePartRef
---@param cost integer From 0 through 2^53-1.
function btech_parts.set_cost(part, cost) end

---Read one part's stored quantity; absent stock reports zero.
---@param target DbRef|Object
---@param part BattlePartRef
---@return integer quantity
function btech_parts.store_quantity(target, part) end

---List positive registered stock rows in native inventory order.
---@param target DbRef|Object
---@return BattlePartStack[] stores
function btech_parts.stores(target) end

-- C-parity repair contracts.

local btech_repair = {}

---@class BattleRepairArmorRequest
---@field operation BattleRepairOperation
---@field section BattleSection
---@field value integer
---@class BattleRepairInternalRequest
---@field operation BattleRepairOperation
---@field section BattleSection
---@field value integer
---@class BattleRepairRearArmorRequest
---@field operation BattleRepairOperation
---@field section BattleSection
---@field value integer
---@class BattleRepairPartRequest
---@field operation BattleRepairOperation
---@field section BattleSection
---@field slot integer
---@class BattleRepairReattachRequest
---@field operation BattleRepairOperation
---@field section BattleSection

---@alias BattleImmediateRepair BattleRepairArmorRequest|BattleRepairInternalRequest|BattleRepairRearArmorRequest|BattleRepairPartRequest|BattleRepairReattachRequest

---Apply one immediate repair with operation from btech.repair.operations.
---@param unit DbRef|Object
---@param repair BattleImmediateRepair
function btech_repair.apply(unit, repair) end

---Report whether no original nonexempt section is destroyed; Mechs exempt all but the
---center torso, ground vehicles exempt the turret and VTOLs exempt the rotor.
---@param unit DbRef|Object
---@return boolean fixable
function btech_repair.is_fixable(unit) end

---Seconds until the player's configured technician becomes available.
---@param player DbRef|Object
---@return integer seconds
function btech_repair.technician_available_in(player) end

-- C-parity world telemetry contracts.

local btech_system = {}

---Seconds of event lag accumulated by the running event queue.
---@return integer seconds
function btech_system.event_lag() end

---List registered BattleTech units contained in a zone.
---@param zone DbRef|Object
---@return Object[] units
function btech_system.units_in_zone(zone) end

-- Typed autopilot constants and unit-attached controller operations.

---@class BattleAutopilotOrderName
---@class BattleAutopilotSubmissionMode
---@class BattleAutopilotFireMode
---@class BattleAutopilotStates
---@field PAUSED "paused"
---@field IDLE "idle"
---@field EXECUTING "executing"
---@field BLOCKED "blocked"
---@class BattleAutopilotOrderStates
---@field QUEUED "queued"
---@field RUNNING "running"
---@field SUCCEEDED "succeeded"
---@field FAILED "failed"
---@field CANCELED "canceled"
---@class BattleAutopilotReasons
---@field MANUAL_TAKEOVER "manual_takeover"
---@field CONTACT_LOST "contact_lost"
---@field STUCK "stuck"
---@field UNREACHABLE "unreachable"
---@field INVALIDATED "invalidated"
---@field RESOURCE_LIMIT "resource_limit"
---@field CONGESTED "congested"
---@field INVALID_TARGET "invalid_target"
---@field UNIT_UNAVAILABLE "unit_unavailable"
---@field MAP_CHANGED "map_changed"
---@field UNSUPPORTED "unsupported"
---@field STALE_REVISION "stale_revision"
---@class BattleAutopilotRangeBand
---@field minimum integer Inclusive minimum engagement range in hexes.
---@field maximum integer Inclusive maximum engagement range in hexes.
---@class BattleAutopilotConfig
---@field speed_percent integer Desired speed as a percentage from 0 through 100.
---@field fire_mode "hold"|"assigned_target"|"opportunistic" Current serialized weapon policy.
---@field heat_ceiling integer Projected heat limit for autonomous fire.
---@field preferred_range BattleAutopilotRangeBand|nil Optional engagement band.
---@class BattleAutopilotConfigPatch
---@field speed_percent integer|nil Optional speed update.
---@field fire_mode BattleAutopilotFireMode|nil Optional weapon-policy update.
---@field heat_ceiling integer|nil Optional projected heat limit.
---@field preferred_range BattleAutopilotRangeBand|false|nil Set or clear the preferred band.
---@alias BattleAutopilotControllerState "paused"|"idle"|"executing"|"blocked"
---@alias BattleAutopilotOrderState "queued"|"running"|"succeeded"|"failed"|"canceled"
---@alias BattleAutopilotFeedbackEvent "configured"|"paused"|"resumed"|"manual_takeover"|"order_queued"|"order_started"|"order_succeeded"|"order_failed"|"order_canceled"|"blocked"
---@alias BattleAutopilotReason "manual_takeover"|"contact_lost"|"stuck"|"unreachable"|"invalidated"|"resource_limit"|"congested"|"invalid_target"|"unit_unavailable"|"map_changed"|"unsupported"|"stale_revision"
---@class BattleAutopilotOrder
---@field kind BattleAutopilotOrderName
---@field destination BattlePosition|nil Move or attack-move destination.
---@field arrival_radius integer|nil Destination tolerance in hexes.
---@field target integer|nil Follow or attack target unit.
---@field separation integer|nil Follow distance in hexes.
---@field waypoints BattlePosition[]|nil Patrol route.
---@field range BattleAutopilotRangeBand|nil Optional attack engagement band.
---@class BattleAutopilotStoredOrder
---@field kind "move"|"hold"|"follow"|"patrol"|"attack"|"attack_move" Serialized intent kind.
---@field destination BattlePosition|nil Move or attack-move destination.
---@field arrival_radius integer|nil Destination tolerance in hexes.
---@field target integer|nil Follow or attack target unit.
---@field separation integer|nil Follow distance in hexes.
---@field waypoints BattlePosition[]|nil Patrol route.
---@field range BattleAutopilotRangeBand|nil Optional engagement band.
---@class BattleAutopilotOrderProgress
---@field waypoint_index integer Current waypoint cursor.
---@field recovery_attempts integer Replanning attempts for the active order.
---@field stagnant_ticks integer Ticks without route progress.
---@field attack_move_origin BattlePosition|nil Position where attack-move pursuit began.
---@field attack_move_suppressed_target integer|nil Contact already engaged during attack-move.
---@class BattleAutopilotOrderRecord
---@field id integer Stable controller-local order ID.
---@field order BattleAutopilotStoredOrder Serialized order intent; submissions use typed constants.
---@field state BattleAutopilotOrderState Lifecycle state.
---@field progress BattleAutopilotOrderProgress Durable execution cursor.
---@class BattleAutopilotStatus
---@field config BattleAutopilotConfig Controller settings.
---@field state BattleAutopilotControllerState Controller lifecycle state.
---@field blocking_reason BattleAutopilotReason|nil Reason the controller is blocked, if any.
---@field revision integer Management revision.
---@field next_order_id integer Next order ID that will be assigned.
---@field active BattleAutopilotOrderRecord|nil Current order.
---@field queue BattleAutopilotOrderRecord[] Queued orders.
---@field feedback BattleAutopilotFeedback[] Recently retained outcomes.
---@field next_feedback_sequence integer Next feedback sequence that will be assigned.
---@field sightings table<integer, BattleAutopilotSighting> Retained contact memory keyed by unit ID.
---@class BattleAutopilotSubmitResult
---@field ids integer[] Assigned order IDs.
---@field revision integer New management revision.
---@class BattleAutopilotContact
---@field unit integer Acquired unit identity.
---@field position BattlePosition Observed position.
---@field friendly boolean Whether the contact is allied.
---@field identified boolean Whether sensors identified the contact well enough to determine allegiance.
---@field known_destroyed boolean Whether the visible contact status reports destruction.
---@field range number Observed range in map units.
---@field seen_at integer Simulation time of the observation.
---@class BattleAutopilotMemory
---@field unit integer Previously acquired unit identity.
---@field position BattlePosition Last sensor-confirmed position.
---@field seen_at integer Simulation time of the last sighting.
---@class BattleHeat
---@field stored number Current stored weapon heat.
---@field excess number Sampled excess heat.
---@class BattleAutopilotOwnReadiness
---@field power BattlePower Current power state.
---@field maximum_speed number Damage-adjusted maximum speed.
---@field heat BattleHeat|nil Conventional heat state; nil for ground vehicles.
---@field weapons BattleWeaponReadiness[] Readiness for installed weapons.
---@class BattleAutopilotSighting
---@field position BattlePosition Last sensor-confirmed position.
---@field seen_at integer Simulation time of the last sighting.
---@class BattleAutopilotObservation
---@field unit integer Observing unit.
---@field time integer Current simulation time.
---@field position BattlePosition|nil Own position, if placed.
---@field heading number|nil Own heading, if motion is available.
---@field speed number Own current speed.
---@field own BattleAutopilotOwnReadiness Own mechanical and weapon readiness.
---@field contacts BattleAutopilotContact[] Current sensor contacts.
---@field remembered BattleAutopilotMemory[] Fresh retained sightings.
---@class BattleAutopilotFeedback
---@field sequence integer Monotonic feedback sequence.
---@field simulation_time integer Simulation time of the event.
---@field order_id integer|nil Related order ID.
---@field event BattleAutopilotFeedbackEvent Event kind.
---@field reason BattleAutopilotReason|nil Optional event reason.
---@class BattleAutopilotFeedbackPage
---@field records BattleAutopilotFeedback[] Retained feedback records after the cursor.
---@field history_gap boolean Whether older records fell outside the retention window.
---@class BtechAutopilotAPI
---@field orders table
---@field submission_modes table
---@field fire_modes table
---@field states BattleAutopilotStates Controller lifecycle strings.
---@field order_states BattleAutopilotOrderStates Order lifecycle strings.
---@field reasons BattleAutopilotReasons Blocking and outcome reason strings.
---@field attach fun(unit: integer, options?: BattleAutopilotConfigPatch)
---@field detach fun(unit: integer)
---@field configure fun(unit: integer, patch: BattleAutopilotConfigPatch, expected_revision?: integer): integer
---@field submit fun(unit: integer, orders: BattleAutopilotOrder[], mode: BattleAutopilotSubmissionMode, expected_revision?: integer): BattleAutopilotSubmitResult
---@field cancel fun(unit: integer, order_id: integer, expected_revision?: integer): boolean
---@field pause fun(unit: integer)
---@field resume fun(unit: integer)
---@field status fun(unit: integer): BattleAutopilotStatus
---@field observe fun(unit: integer): BattleAutopilotObservation
---@field feedback fun(unit: integer, after_sequence?: integer): BattleAutopilotFeedbackPage

local btech_autopilot = {} ---@type BtechAutopilotAPI

btech.parts = btech_parts
btech.repair = btech_repair
btech.system = btech_system
btech.autopilot = btech_autopilot

---@class BattleTacticalUnitSnapshot
---@field unit integer Assigned friendly unit ID.
---@field revision integer Management revision used for stale-intention protection.
---@field status BattleAutopilotStatus Controller state; sightings are supplied through observation instead.
---@field observation BattleAutopilotObservation Per-unit permitted intelligence.
---@field feedback BattleAutopilotFeedbackPage Outcome page after the requested cursor.
---@class BattleTacticalSighting
---@field observer integer Unit that acquired this sighting.
---@field position BattlePosition Last observed position.
---@field seen_at integer Committed simulation seconds.
---@field current boolean Whether this observer currently acquires the contact.
---@field friendly boolean|nil Present only for a current observation.
---@field identified boolean|nil Present only for a current observation.
---@field known_destroyed boolean|nil Present only for a current observation.
---@class BattleTacticalContact
---@field unit integer Contact identity.
---@field observations BattleTacticalSighting[] Source observations, ordered by observer ID.
---@class BattleTacticalSnapshot
---@field version integer Snapshot schema version, currently 1.
---@field time integer Committed simulation seconds; restart does not advance this clock.
---@field units BattleTacticalUnitSnapshot[] Assigned controllers, ordered by unit ID.
---@field contacts BattleTacticalContact[] Aggregated sightings, ordered by contact ID.
---@class BattleTacticalIntention
---@field unit integer Assigned unit ID.
---@field expected_revision integer Required current management revision.
---@field mode BattleAutopilotSubmissionMode Append or replace using typed constants.
---@field orders BattleAutopilotOrder[] Ordinary unit orders, at most 64.
---@class BattleTacticalSubmitResult : BattleAutopilotSubmitResult
---@field unit integer Controller receiving these order IDs.
---@class BtechTacticalAPI
local btech_tactical = {}
---Read a detached tactical snapshot for 1 to 100 distinct friendly controllers on one map.
---Shared sightings retain observer provenance and do not grant another unit attack admission.
---@param units integer[] Explicit assigned unit IDs; every unit must be attached and placed.
---@param feedback_cursors table<integer, integer>|nil Optional per-unit feedback sequence cursors.
---@return BattleTacticalSnapshot snapshot Versioned intelligence and controller outcomes.
function btech_tactical.observe(units, feedback_cursors) end
---Atomically validate and submit intentions for 1 to 100 distinct friendly controllers.
---Any invalid order or stale revision rejects the whole batch. Paused units stay paused.
---@param intentions BattleTacticalIntention[] One intention per unit; revisions are required.
---@return BattleTacticalSubmitResult[] results Assigned IDs and revisions in request order.
function btech_tactical.submit(intentions) end
btech.tactical = btech_tactical

return btech
