---
title: "btech value types"
type: docs
---

Record shapes and aliases used by the callable signatures in this reference.

## MechSectionName

Alias: `"LeftArm"|"RightArm"|"LeftTorso"|"RightTorso"|"CenterTorso"|"LeftLeg"|"RightLeg"|"Head"`

## Notice

- `unit`: `integer` — Recipient unit dbref.
- `text`: `string` — Cockpit message text.

## CriticalDefinition

- `equipment`: `string` — Unresolved asset equipment name.
- `data`: `string` — Unresolved asset data token.
- `modes`: `string[]` — Unresolved asset mode names.

## SectionDefinition

- `armor`: `integer`
- `internal`: `integer`
- `rear`: `integer`
- `criticals`: `table<integer, CriticalDefinition>` — Zero-based slot positions.
- `configuration`: `string|nil`

## MechTemplate

- `name`: `string`
- `reference`: `string`
- `tons`: `integer`
- `max_speed`: `number`
- `jump_speed`: `number`
- `heat_sinks`: `integer` — Cooling capacity; double sinks are already counted twice.
- `sections`: `table<MechSectionName, SectionDefinition>`
- `attributes`: `table<string, string>` — Unit-level source fields; not validated simulation capabilities.

## MapAssetSummary

- `width`: `integer`
- `height`: `integer`
- `gravity`: `integer`
- `temperature`: `integer`
- `flags`: `MapFlag[]` — Map flags the asset enables.
- `points_of_interest`: `MapPointOfInterest[]` — Scripted points of interest in file order.

## Hex

- `level`: `integer` — Ground height in levels; any water surface sits at this height.
- `ground`: `GroundName` — What the ground is made of; see btech.map.ground_types.
- `woods`: `WoodsName` — Forest covering the ground; see btech.map.woods_types.
- `water`: `{depth: integer, frozen: boolean}` — Standing water whose surface is at the ground level.
- `structure`: `{kind: "building"|"wall", height: integer}|{kind: "bridge", deck: integer}` — Built feature; heights are above the ground level. Kinds are in btech.map.structure_kinds.
- `overlay`: `"fire"|"smoke"` — Fire or smoke over the hex; base tiles never have one.

## StoredMap

- `cargo_transfer_point`: `CargoTransferPoint|nil` — Saved cargo location and hint policy.
- `building_exits`: `table<integer, {coordinate: HexCoordinate, destination: integer, data_char: integer, data_short: integer, data_int: integer}>` — Return-link slots; coordinates are selection metadata.
- `name`: `string`
- `width`: `integer`
- `height`: `integer`
- `gravity`: `integer`
- `temperature`: `integer`

## StoredBattleUnit

- `name`: `string`
- `template`: `string`
- `class_code`: `integer` — Saved unit class, including deferred classes.
- `movement_code`: `integer` — Saved movement type, including deferred types.
- `tons`: `integer`
- `map`: `integer|nil`

## Weapon

Alias: `"arrow_iv"|"clan_arrow_iv"|"long_tom"|"sniper"|"thumper"|"long_tom_cannon"|"sniper_cannon"|"thumper_cannon"|"a_pod"|"clan_a_pod"|"i_narc_beacon"|"narc_beacon"|"clan_narc_beacon"|"anti_missile_system"|"clan_anti_missile_system"|"laser_ams"|"clan_laser_ams"|"clan_lbx2"|"clan_lbx5"|"clan_lbx10"|"clan_lbx20"|"clan_ultra_ac2"|"clan_ultra_ac5"|"clan_ultra_ac10"|"clan_ultra_ac20"|"mml3"|"mml5"|"mml7"|"mml9"|"clan_atm3"|"clan_atm6"|"clan_atm9"|"clan_atm12"|"clan_lrm5"|"clan_lrm10"|"clan_lrm15"|"clan_lrm20"|"clan_srm2"|"clan_srm4"|"clan_srm6"|"clan_streak_srm2"|"clan_streak_srm4"|"clan_streak_srm6"|"clan_streak_lrm5"|"clan_streak_lrm10"|"clan_streak_lrm15"|"clan_streak_lrm20"|"clan_gauss_rifle"|"clan_machine_gun"|"clan_light_machine_gun"|"clan_heavy_machine_gun"|"clan_er_large_laser"|"clan_er_medium_laser"|"clan_er_small_laser"|"clan_er_micro_laser"|"clan_er_ppc"|"clan_flamer"|"clan_heavy_large_laser"|"clan_heavy_medium_laser"|"clan_heavy_small_laser"|"clan_large_pulse_laser"|"clan_medium_pulse_laser"|"clan_small_pulse_laser"|"clan_micro_pulse_laser"|"clan_er_large_pulse_laser"|"clan_er_medium_pulse_laser"|"clan_er_small_pulse_laser"|"clan_plasma_rifle"|"flamer"|"coolant_gun"|"heavy_flamer"|"vehicle_flamer"|"vehicle_heavy_flamer"|"plasma_rifle"|"acid_thrower"|"thunderbolt5"|"thunderbolt10"|"thunderbolt15"|"thunderbolt20"|"hyper_ac2"|"hyper_ac5"|"hyper_ac10"|"machine_gun"|"heavy_machine_gun"|"light_ac2"|"light_ac5"|"small_laser"|"medium_laser"|"large_laser"|"ppc"|"er_small_laser"|"er_medium_laser"|"er_large_laser"|"er_ppc"|"small_pulse_laser"|"medium_pulse_laser"|"large_pulse_laser"|"x_small_pulse_laser"|"x_medium_pulse_laser"|"x_large_pulse_laser"|"light_ppc"|"heavy_ppc"|"snub_nosed_ppc"|"srm2"|"rocket10"|"rocket15"|"rocket20"|"mrm10"|"mrm20"|"mrm30"|"mrm40"|"streak_srm2"|"streak_srm4"|"streak_srm6"|"lr_dfm5"|"lr_dfm10"|"lr_dfm15"|"lr_dfm20"|"sr_dfm2"|"sr_dfm4"|"sr_dfm6"|"elrm5"|"elrm10"|"elrm15"|"elrm20"|"nlrm5"|"nlrm10"|"nlrm15"|"nlrm20"|"lrt5"|"lrt10"|"lrt15"|"lrt20"|"srt2"|"srt4"|"srt6"|"clan_lrt5"|"clan_lrt10"|"clan_lrt15"|"clan_lrt20"|"clan_srt2"|"clan_srt4"|"clan_srt6"|"lrm5"|"lrm10"|"lrm15"|"srm4"|"srm6"|"lrm20"|"heavy_gauss_rifle"|"gauss_rifle"|"light_gauss_rifle"|"magshot_gauss_rifle"|"lbx2"|"lbx5"|"lbx10"|"lbx20"|"ac2"|"ac5"|"ac10"|"ac20"|"ultra_ac2"|"ultra_ac5"|"ultra_ac10"|"ultra_ac20"|"rotary_ac2"|"rotary_ac5"|"clan_rotary_ac2"|"clan_rotary_ac5"|"clan_rotary_ac10"`

## CriticalLocation

- `section`: `MechSectionName`
- `slot`: `integer` — Zero-based critical slot.

## AmmunitionMode

Alias: `"smoke"|"mine"|"i_narc_explosive"|"i_narc_haywire"|"i_narc_ecm"|"i_narc_nemesis"|"semi_guided"|"swarm"|"swarm1"|"stinger"|"narc"|"normal"|"cluster"|"artemis"|"precision"|"flechette"|"armor_piercing"|"caseless"|"incendiary"|"inferno"|"mml_lrm"|"mml_lrm_artemis"|"mml_lrm_narc"|"mml_lrm_swarm"|"mml_lrm_swarm1"|"mml_lrm_semi_guided"|"mml_lrm_stinger"|"extended_range"|"high_explosive"|"thunder_augmented"|"thunder_vibrabomb"|"thunder_active"`

## FireMode

Alias: `"normal"|"heat"|"hotload"|"ultra"|"rapid"|"rotary2"|"rotary3"|"rotary4"|"rotary5"|"rotary6"|"gatling"`

## WeaponMount

- `weapon`: `Weapon`
- `criticals`: `CriticalLocation[]` — Slots belonging to one weapon.
- `one_shot`: `boolean` — Self-contained launcher; does not draw from ammunition bins.
- `initially_spent`: `boolean` — Initial template supply already expended.
- `initial_ammunition_mode`: `AmmunitionMode`
- `initial_fire_mode`: `FireMode` — Template mode; live mode is reported by unit.weapons.
- `rear_mount`: `boolean`
- `on_targeting_computer`: `boolean` — Explicit authored link, separate from automatic eligibility.

## AmmunitionBin

- `location`: `CriticalLocation`
- `weapon`: `Weapon`
- `rounds`: `integer` — Initial salvos in this independent bin.
- `capacity`: `integer` — Installed bin capacity in salvos.
- `hotload`: `boolean` — Retained bin flag; does not hotload the launcher.
- `half_ton`: `boolean` — Explicit half-ton construction flag.
- `mode`: `AmmunitionMode`

## SystemCritical

- `location`: `CriticalLocation`
- `system`: `string` — Snake_case system identity.

## MechLoadout

- `weapons`: `WeaponMount[]`
- `ammunition`: `AmmunitionBin[]`
- `systems`: `SystemCritical[]`

## CargoTransferPoint

- `x`: `integer` — Zero-based map column.
- `y`: `integer` — Zero-based map row.
- `reveal_hint`: `boolean|nil` — Defaults to false; disclose coordinates in location failures only when true.

## RadioChannel

- `frequency`: `integer` — Frequency from 0 through 999999.
- `title`: `string` — At most fifteen UTF-8 bytes.
- `mode`: `{digital: boolean, muted: boolean, relay: boolean, info: boolean, scan: boolean, color: string?}`

## VtolFuelStatus

- `original_capacity`: `integer` — Template fuel capacity.
- `capacity`: `integer` — Current capacity including 2000 per installed or carried auxiliary tank.
- `remaining`: `integer` — Saved fuel; -1 indicates announced exhaustion.
- `auxiliary_tanks`: `integer` — Carried Fuel_Tank items.
- `installed_tanks`: `integer` — Fuel_Tank criticals in saved VTOL construction.
- `excess_mass`: `integer` — Fuel above original capacity in 1/1024 tons before cargo discounts.

## SectionState

- `armor`: `integer`
- `internal`: `integer`
- `rear`: `integer`

## Position

- `map`: `integer` — Battlefield object dbref.
- `x`: `integer` — Zero-based column.
- `y`: `integer` — Zero-based row.

## UnitPower

- `state`: `"off"|"starting"|"running"`
- `remaining`: `integer|nil` — Remaining committed seconds during startup.

## Point

- `x`: `number`
- `y`: `number`

## Motion

- `point`: `table` — Continuous x/y measured in hex heights.
- `heading`: `number` — Current clockwise compass heading.
- `desired_heading`: `number`
- `speed`: `number` — Current kph, negative for reverse.
- `desired_speed`: `number`

## Mobility

- `maximum_speed`: `number` — Damage-adjusted maximum kph before terrain, heat and cargo.
- `piloting_modifier`: `integer` — Damage modifier for subsequent piloting checks.

## DetectionChannel

Alias: `"sensors"|"sight"|"radar"|"probe" Values from btech.unit.detection_channels.`

## PerceptionStatus

Alias: `"ready"|"degraded"|"jammed"|"damaged"|"disabled"|"absent"`

## TargetLock

- `target`: `integer` — Selected unit dbref; may no longer be visible.
- `remaining`: `integer` — Settling seconds, 0..8; zero does not establish visibility.

## StaggerHit

- `damage`: `integer`
- `remaining`: `integer` — Seconds until this incoming damage group expires.
- `counted`: `boolean` — Whether a rolling check already used this group.

## Stagger

- `action_damage`: `integer` — Restored signed action-time scalar, independent of incoming damage history.
- `hits`: `StaggerHit[]`
- `elapsed`: `integer` — Committed seconds since the last rolling check.
- `turn_damage`: `integer` — Unchecked traditional damage.
- `phase`: `integer` — Saved per-unit turn phase, 0 through 29.
- `checked_phase`: `integer?` — Phase of the previous traditional check.

## StandTimer

- `state`: `"rising"|"recovering"`
- `remaining`: `integer` — Committed seconds remaining, 1..60.

## Mass

- `engine`: `integer` — Engine mass in 1/1024 tons.
- `cockpit`: `integer`
- `gyro`: `integer` — Signed gyro accounting for destroyed center torsos.
- `structure`: `integer`
- `armor`: `integer`
- `equipment`: `integer`
- `ammunition`: `integer`
- `cargo`: `integer` — Authored cargo-space installation mass, excluding loose stock.
- `total`: `integer` — Current total in 1/1024 tons.

## LateralMode

Alias: `"none" | "front_left" | "front_right" | "rear_left" | "rear_right"`

## LateralState

- `active`: `LateralMode`
- `pending`: `LateralMode?`
- `remaining`: `integer` — Seconds until pending direction activates.

## TransportState

- `altitude`: `number|nil` — Continuous altitude in terrain levels, including carried and pending-descent fractions.
- `fortified`: `boolean` — Scenario emplacement; blocks movement and towing, counts as immobile for aiming.
- `towable`: `boolean` — Explicit permission to tow this unit out of character.
- `towing`: `integer|nil` — Unit currently carried by this unit.
- `towed_by`: `integer|nil` — Carrier currently towing this unit.
- `orbital_drop`: `{elevation: integer, protection: {state: "cocoon"|"jump_jets", integrity: integer?}}|nil` — Saved orbital descent; inspection does not advance it.
- `free_fall`: `{elevation: number, speed: integer, remaining: integer, grounded: boolean}|nil` — Pending descent; inspection does not advance it.

## HullDownState

- `active`: `boolean` — Completed lowered posture.
- `pending`: `boolean|nil` — Lowering (true) or raising (false).
- `remaining`: `integer` — Seconds left in the transition; zero when idle.

## RadioState

- `radio_experience_remaining`: `integer` — Saved communication XP gate, 0 through 61 seconds.
- `radio_skill`: `integer` — Communication target captured on startup.
- `radio`: `RadioChannel[]` — Active channels, indexed from one in this inspection array.
- `radio_capabilities`: `{channels: integer, range: integer, relay: boolean, digital: boolean, info: boolean, scan: boolean}` — Derived installed hardware limits.

## UnitState

- `mw_safety`: `boolean` — MechWarrior safety; enabled when startup completes.
- `bth_debug`: `boolean` — Retained debug preference; combat reports do not consume this flag.
- `last_startup`: `integer` — Unix time of the last completed startup; zero before first completion.
- `cockpit_links`: `integer[]` — Three explicit cockpit destinations; unresolved references remain saved.
- `preferred_id`: `string?` — Configured two-letter preference; separate from the currently assigned ID.
- `hull_down`: `HullDownState` — Quad hull-down posture and transition.
- `kind`: `"mech"`
- `mass`: `Mass` — Derived current mass; detached from world state.
- `searchlight_warning`: `boolean` — Notify occupants on external illumination transitions.
- `lateral`: `LateralState`
- `autocon_shutdown`: `boolean` — Include shutdown targets in routine notices; defaults false.
- `armor_warning`: `boolean` — Armor threshold warnings; enabled by default.
- `ammunition_warning`: `boolean` — Low-ammunition warnings; enabled by default.
- `friendly_fire_safety`: `boolean` — Reject non-coolant fire at teammates; off by default.
- `null_signature`: `SignatureState`
- `stealth`: `SignatureState`
- `electronics`: `Electronics` — Selected suite modes and last committed field.
- `beacons`: `table<MechSection, BeaconKind[]>` — Attached effects grouped by section.
- `narc_sections`: `MechSection[]` — Sections carrying homing beacons.
- `ams_enabled`: `boolean` — Automatic anti-missile defense switch.
- `auto_fall`: `boolean` — Skip downhill cliff avoidance when piloted.
- `hex_sync_pending`: `boolean` — A collision interrupted synchronization of motion.point and position.
- `elevation`: `integer|nil` — Current signed altitude with terrain-effect jump rounding; nil when unplaced.
- `stand_timer`: `StandTimer?`
- `reactor_instability_remaining`: `integer?` — Damage window ticks remaining; nil uses initial world startup grace.
- `triple_myomer_active`: `boolean` — Derived from installed myomer and sampled excess heat.
- `movement_maximum_speed`: `number` — Current throttle ceiling, including active myomer.
- `charge`: `{target: integer?, elapsed: integer, distance: number}` — Persistent charge intent and movement counters.
- `limb_recycle`: `table<string, integer>` — Remaining physical recovery seconds by limb.
- `stagger`: `Stagger`
- `posture`: `"standing"|"prone"`
- `flooded_sections`: `string[]` — Persistent flooded section names.
- `breached_sections`: `string[]` — Persistent vacuum-disabled section names.
- `map_slot`: `integer|nil` — Persisted battlefield membership order.
- `aimed_section`: `AimSelection|nil` — Saved anatomy preference; independent of the current lock.
- `target_lock`: `TargetLock|HexLock|nil`
- `sensor_ranges`: `{tactical: integer, long_range: integer, scan: integer}` — Computer-derived hex limits after sensor damage.
- `observer`: `boolean` — Administrator-assigned observer role.
- `combat_safe`: `boolean` — Operator-imposed immunity to combat damage.
- `weapons_hold`: `boolean` — Operator-imposed firing restriction; mechanical readiness is independent.
- `visibility`: `{invisible: boolean, clairvoyant: boolean}` — Operator visibility state.
- `battlefield_id`: `string?` — Current battlefield identity; absent without map membership.
- `searchlight`: `{on: boolean, destroyed: boolean, remaining: integer, mode: "auto"|"on"|"off"}` — Hardware, pending five-second switch and switching policy.
- `fired_recently`: `boolean` — Launched a weapon since the last heartbeat.
- `spotter`: `integer?` — Self ID while spotting, otherwise the selected observer.
- `artillery_adjustment`: `integer` — Saved correction for the current artillery target.
- `spotter_events`: `SpotterEvents` — Pending radio requests and periodic checks.
- `tag`: `TagState`
- `signature`: `{team: integer, hidden: boolean, illuminated: boolean}` — Team, hiding and scenario lighting.
- `scanner_perception`: `integer` — Perception captured at startup completion.
- `facing`: `{torso: "left"|"center"|"right"|"both", arms_flipped: boolean}`
- `stun_remaining`: `integer` — Remaining seconds of cockpit stun.
- `pilot_injuries`: `integer` — Tactical injury count; six means scenario pilot loss.
- `self_destruct`: `{remaining: integer, ammunition: boolean}|nil` — Admitted timer and its actual Mech detonation mode.
- `self_destruct_safe`: `boolean` — Scenario protection from new ammunition self-destruct requests.
- `hide_elapsed`: `integer|nil` — Elapsed camouflage checks; nil when no hide event is pending.
- `crew_recovery_remaining`: `integer` — Empty-crew consciousness countdown; random state stays private.
- `character_pilot`: `{injuries: integer, killed: boolean}?` — Saved character-mode injury status; character health determines death.
- `heat_cutoff`: `{enabled: boolean, disabled: integer, remaining: integer|nil}` — Intentional cooling suppression and seconds until the toggle completes.
- `last_jump`: `{heading: integer, length: integer}` — Current course bearing and signed length in field units, retained after landing.
- `heat_sample`: `{production: number, dissipation: number}` — Last committed thermal sample; production includes stored weapon heat.
- `heat`: `{stored: number, excess: number}` — Weapon heat (possibly negative coolant credit until the next sample) and sampled excess heat.
- `inferno_remaining`: `integer` — Saved burn seconds; cooling is reduced by six while positive.
- `overheat_clock`: `{elapsed: integer, phase: integer, injury_due: boolean}` — Saved committed-second thermal checks.
- `weapon_recycle`: `table<integer, integer>` — Remaining seconds keyed by zero-based weapon index.
- `component_failures`: `{location: table, failure: string}[]` — Nonweapon diagnostic conditions; material damage determines system operation.
- `weapon_failures`: `table<integer, "jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam">` — Temporary conditions by mount index; existing recycle clocks govern recovery.
- `gyro`: `"standard"|"hardened"|"xl"|"compact"` — Construction family.
- `artemis`: `ArtemisController[]` — Installed controllers and resolved links.
- `weapon_damage`: `WeaponDamage[]|nil` — Mech weapon critical degradation.
- `masc`: `BoosterState` — Saved activation and overload/recovery state.
- `supercharger`: `BoosterState` — Independent compressor timer and failure state.
- `supercharger_installed`: `boolean` — Template technology flag.
- `supercharger_operational`: `boolean` — Technology remains available and has not failed.
- `c3_members`: `integer[]` — Classic C3 members retained by current working-master capacity.
- `c3_operational`: `boolean` — Working classic C3 hardware.
- `c3i_members`: `integer[]` — Eligible network members including this unit, empty when disconnected; shutdown and ECM retain membership.
- `c3_hardware`: `{masters: integer, working_masters: integer, slave_installed: boolean, slave_operational: boolean, c3i_installed: boolean, c3i_operational: boolean}` — Installed and working command-network computers; independent of power and membership.
- `masc_installed`: `boolean` — Sufficient MASC hardware is installed.
- `masc_operational`: `boolean` — Enough MASC slots remain functional; does not indicate activation.
- `unjam`: `Unjam|nil` — Active feed recovery.
- `dumping`: `table|nil` — Active ammunition selection and elapsed cadence.
- `gyro_damage`: `integer` — Effective gyro damage after hardened protection.
- `mobility`: `Mobility`
- `jump_capacity`: `{speed: number, movement_points: integer}` — Damage/gravity-adjusted capacity; does not authorize flight. Unplaced units use 100% gravity.
- `flight`: `{path: {start: Point, end: Point, start_elevation: number, end_elevation: integer, movement_points: integer, continuation: boolean, projection: {bearing: integer, range: number}|nil, target_range: number|nil}, travelled: number, completed_distance: number, landing_requested: boolean, sampled_movement_points: integer, dfa_target: integer|nil}|nil`
- `airborne`: `{point: Point, elevation: number}|nil` — Last committed airborne sample.
- `jump_stabilization`: `integer` — Remaining seconds, zero through twelve.
- `engine`: `"standard"|"light"|"xl"|"xxl"|"compact"` — Installed fusion-engine family.
- `destroyed`: `boolean` — Core structure, cockpit or engine is destroyed.
- `lost_criticals`: `CriticalLocation[]` — Explicit destroyed equipment slots.
- `motion`: `Motion|nil`
- `power`: `UnitPower`
- `pilot`: `integer|nil` — Player in the cockpit; must be physically inside this unit.
- `position`: `Position|nil`
- `definition`: `MechTemplate` — Owned definition, independent of source files.
- `sections`: `table<MechSectionName, SectionState>`
- `ammunition`: `integer[]` — Remaining salvos in resolved bin order.

## VehicleMass

- `engine`: `integer`
- `cockpit`: `integer`
- `components`: `integer`
- `turret`: `integer`
- `structure`: `integer`
- `armor`: `integer`
- `equipment`: `integer`
- `cooling`: `integer`
- `cargo`: `integer`
- `ammunition`: `integer` — Loaded ammunition mass.
- `ammunition_capacity`: `integer` — Full surviving bin mass.
- `total`: `integer` — Current physical mass in 1/1024-ton units.
- `design_total`: `integer` — Current component total with full surviving bins.

## DigState

- `dug_in`: `boolean` — Whether cover applies.
- `digging`: `boolean` — Whether preparation is active.
- `completion`: `integer[]` — Pending completion deadlines in seconds; empty means none.

## VehicleState

- `armor_warning`: `boolean` — Armor severity warnings; enabled by default.
- `ammunition_warning`: `boolean` — Low-ammunition warnings; enabled by default.
- `searchlight`: `{on: boolean, destroyed: boolean, remaining: integer, mode: "auto"|"on"|"off"}` — Hardware, pending five-second switch and switching policy.
- `autocon_shutdown`: `boolean` — Include shutdown targets in routine contact notices.
- `searchlight_warning`: `boolean` — Announce external illumination transitions.
- `mw_safety`: `boolean` — MechWarrior safety; enabled when startup completes.
- `bth_debug`: `boolean` — Retained debug preference; combat reports do not consume this flag.
- `last_startup`: `integer` — Unix time of the last completed startup; zero before first completion.
- `cockpit_links`: `integer[]` — Three explicit cockpit destinations; unresolved references remain saved.
- `preferred_id`: `string?` — Configured two-letter preference; separate from the currently assigned ID.
- `fuel`: `VtolFuelStatus|nil` — Live fuel projection for VTOLs only.
- `fired_recently`: `boolean` — A weapon launched since the last heartbeat.
- `observer`: `boolean` — Administrator-assigned observer role.
- `combat_safe`: `boolean` — Operator-imposed immunity to combat damage.
- `weapons_hold`: `boolean` — Operator-imposed firing restriction; mechanical readiness is independent.
- `visibility`: `{invisible: boolean, clairvoyant: boolean}` — Operator visibility state.
- `dig`: `DigState` — Saved ground-vehicle cover preparation.
- `mass`: `VehicleMass` — Derived from current material and ammunition; units are 1/1024 ton.
- `inferno_remaining`: `integer` — Stationary-unit jelly duration.
- `burning_sections`: `table<string, integer>` — Section fire countdowns in seconds.
- `extinguishing`: `integer|nil` — Seconds until the crew completes its attempt.
- `pod_removal`: `integer|nil` — Remaining seconds of the crew iNarc-removal attempt.
- `beacons`: `table<string, string[]>` — Attached effects keyed by surviving vehicle section.
- `ams_enabled`: `boolean` — Saved automatic anti-missile defense switch.
- `artemis`: `ArtemisController[]` — Installed controllers and resolved links.
- `unjam`: `Unjam|nil` — Active feed clearing attempt.
- `weapon_recycle`: `table<integer, integer>` — Countdown seconds by zero-based weapon index.
- `spent_launchers`: `integer[]` — Expended zero-based one-shot weapon indices.
- `lost_criticals`: `table[]` — Destroyed vehicle equipment locations, each with section and zero-based slot.
- `piloting_damage`: `integer` — Cumulative vehicle handling penalty.
- `component_failures`: `{location: table, failure: string}[]` — Nonweapon diagnostic conditions; material damage determines system operation.
- `weapon_failures`: `table<integer, "jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam">` — Temporary conditions by mount index; existing recycle clocks govern recovery.
- `crew_stun_remaining`: `integer` — Seconds until the pending recovery event; zero means none.
- `crew_stunned`: `boolean` — Effective crew stun, independent of its timer.
- `self_destruct`: `{remaining: integer, ammunition: boolean}|nil` — Admitted timer and its actual Mech detonation mode.
- `self_destruct_safe`: `boolean` — Scenario protection from new ammunition self-destruct requests.
- `hide_elapsed`: `integer|nil` — Elapsed camouflage checks; nil when no hide event is pending.
- `crew_recovery_remaining`: `integer` — Empty-crew consciousness countdown, separate from crew stun.
- `weapon_heat`: `number` — Passive weapon heat and coolant credit; ground vehicles do not overheat.
- `gunnery_damage`: `integer` — Cumulative firing penalty from sensor and commander damage.
- `lost_stabilizers`: `string[]` — Sections with destroyed weapon stabilizers.
- `signature`: `{team: integer, hidden: boolean, illuminated: boolean}` — Team, hiding and scenario lighting.
- `scanner_perception`: `integer` — Perception captured at startup completion.
- `sensor_ranges`: `{tactical: integer, long_range: integer, scan: integer}` — Computer-derived hex limits.
- `aimed_section`: `AimSelection|nil` — Saved anatomy preference; independent of the current lock.
- `target_lock`: `TargetLock|HexLock|nil` — Saved selection and settling countdown.
- `artillery_adjustment`: `integer` — Saved correction for the selected artillery coordinate.
- `c3_hardware`: `{masters: integer, working_masters: integer, slave_installed: boolean, slave_operational: boolean, c3i_installed: boolean, c3i_operational: boolean}` — Installed and working command-network computers; independent of power and membership.
- `c3i_members`: `integer[]` — Eligible members in the improved command network.
- `c3_members`: `integer[]` — Eligible members in the classic command network.
- `flooded`: `boolean` — Permanently disabled by water, independently of armor and crew health.
- `breached_sections`: `string[]` — Persisted vacuum breaches; equipment is disabled without destroying slots or expending ammunition.
- `crew_killed`: `boolean` — Instant crew loss, independent of tactical and character injury counts.
- `electronics`: `Electronics` — Selected suite modes and last committed field.
- `spotter`: `integer|nil` — Self declares spotting; another unit selects a forward observer.
- `spotter_events`: `SpotterEvents` — Pending radio requests and periodic checks.
- `tag`: `TagState` — Shared TAG selection and lock/recycle countdown.
- `character_pilot`: `{injuries: integer, killed: boolean}?` — Saved RPG pilot status, independent of tactical injury count.
- `friendly_fire_safety`: `boolean` — Pilot-selected teammate protection.
- `auto_fall`: `boolean` — Skip downhill cliff avoidance when piloted.
- `brief`: `BriefSettings`
- `fire_modes`: `table<integer, FireMode>` — Selected non-normal firing modes by zero-based weapon index.
- `ammunition_modes`: `table<integer, AmmunitionMode>` — Selected non-normal modes by zero-based weapon index.
- `turret_heading`: `number|nil` — Absolute heading of a surviving turret.
- `automatic_turret`: `boolean` — Pilot-selected automatic unit/hex target tracking.
- `turret_jammed`: `boolean` — Recoverable turret rotation damage.
- `turret_repairs`: `integer[]` — Pending 60-second repair attempts.
- `turret_locked`: `boolean` — Turret damage prevents rotation.
- `maximum_speed`: `number` — Current maximum kph after motive damage.
- `motive_speed_loss`: `number` — Maximum speed lost to motive damage in kph.
- `immobilized`: `boolean` — Motive-system destruction prevents ground motion.
- `under_bridge`: `boolean` — Hovercraft beneath a bridge span.
- `elevation`: `integer|nil` — Ground support height; hovercraft float at water level.
- `motion`: `Motion|nil`
- `pilot`: `integer|nil` — Assigned cockpit operator.
- `power`: `UnitPower`
- `kind`: `"vehicle"`
- `simulation_supported`: `false` — Full vehicle terrain and combat support is unfinished.
- `definition`: `table` — Owned ground-vehicle definition.
- `sections`: `table<string, SectionState>` — Vehicle faces: left, right, front, rear, turret.
- `ammunition`: `integer[]` — Remaining rounds in resolved bin order.
- `position`: `Position|nil`
- `map_slot`: `integer|nil`
- `destroyed`: `boolean` — Any hull face has lost its internal structure.

## Range

- `horizontal`: `number` — Horizontal Euclidean range in hex heights.
- `spatial`: `number` — Euclidean range including signed ground elevation/depth.
- `bearing`: `number|nil` — Degrees clockwise from north; nil for coincident centers.
- `hex_distance`: `integer` — Minimum adjacent hex steps, without terrain costs.

## Character

- `perception_target`: `integer` — Target derived from intuition, learning and effective Perception skill.
- `values`: `table<string, {value: integer, experience: integer, last_used: integer}>` — Detached named skill/advantage records.
- `unconscious_remaining`: `integer` — Seconds before the next recovery attempt; zero when conscious.
- `bruise`: `integer`
- `lethal`: `integer`
- `build`: `integer`
- `reflexes`: `integer`
- `intuition`: `integer`
- `learn`: `integer`
- `charisma`: `integer`

## AdvantageDefinition

- `name`: `string` — Canonical advantage name.
- `kind`: `"boolean"|"ranked"|"attribute_mask"` — Boolean values activate only at one.

## SkillDefinition

- `name`: `string` — Canonical storage name.
- `category`: `"athletic"|"mental"|"physical"|"social"`
- `threshold`: `integer` — Default experience threshold.
- `continuous`: `boolean` — Whether awards bypass the thirty-second interval.

## SkillProgress

- `name`: `string` — Canonical skill name.
- `target`: `integer` — Current skill target including stored earned levels.
- `raw_target`: `integer` — Skill target excluding earned levels.
- `earned_levels`: `integer` — Stored XP bonus.
- `balance`: `integer` — Low 24-bit experience balance.
- `threshold`: `integer` — Current runtime threshold.
- `next_level_balance`: `integer?` — Total balance needed for the next stored level; nil when disabled.
- `remaining`: `integer?` — Additional points needed; zero if recalculation is overdue.

## HexCoordinate

- `x`: `integer`
- `y`: `integer`

## SurfaceBreak

- `map`: `integer`
- `coordinate`: `HexCoordinate`
- `before`: `Hex`
- `after`: `Hex`
- `fall_levels`: `integer`
- `falls`: `table[]` — Ordered pairs of unit dbref and Mech fall report.
- `vehicle_falls`: `table[]` — Ordered pairs of unit dbref and vehicle fall report.
- `flooded_vehicles`: `integer[]`
- `notices`: `table[]` — Unit dbrefs and cockpit message text.

## MapEmitOptions

- `audience`: `"all"|"range"|"line_of_sight"` — Recipient selection; defaults to all.
- `origin`: `HexCoordinate` — Required anchor for range and line_of_sight audiences.
- `range`: `number` — Nonnegative hex radius; required with the range audience.

## AuthoredMapLink

- `parent`: `integer` — Parent map.
- `coordinate`: `HexCoordinate` — Placement on the parent.
- `entrances`: `table[]` — Four cardinal modes, north/east/south/west: {kind="none"}, {kind="offset",distance=N}, or {kind="exact",coordinate={x=X,y=Y}}.

## MapEntrance

Alias: `{mode: "offset", offset: integer}|{mode: "exact", x: integer, y: integer}`

## MapEntrances

- `north`: `MapEntrance`
- `east`: `MapEntrance`
- `south`: `MapEntrance`
- `west`: `MapEntrance`

## MapLink

- `parent`: `Object` — Parent map object.
- `x`: `integer` — Placement column on the parent.
- `y`: `integer` — Placement row on the parent.
- `entrances`: `MapEntrances`

## MapHexChange

- `map`: `integer`
- `coordinate`: `HexCoordinate`
- `before`: `Hex`
- `after`: `Hex`

## MapIceReport

- `map`: `integer`
- `changed`: `HexCoordinate[]` — Coordinates in column-major processing order.
- `fractures`: `SurfaceBreak[]` — Melting consequences, including affected occupants.

## MapEnvironment

- `gravity`: `integer` — Percent of Earth gravity, 0 through 255.
- `temperature`: `integer` — Celsius, -128 through 127.
- `vacuum`: `boolean?` — Defaults to false, clearing existing vacuum.
- `underground`: `boolean?` — Defaults to false; existing underground status is retained.

## BlastZone

- `x`: `integer`
- `y`: `integer`
- `radius`: `integer`

## TerrainName

Alias: `"grassland"|"road"|"light_forest"|"heavy_forest"|"water"|"ice"|"bridge"|"rough"|"mountains"|"fire"|"smoke"|"snow"|"building"|"wall"|"sand"`

## TerrainTypes

- `GRASSLAND`: `"grassland"`
- `ROAD`: `"road"`
- `LIGHT_FOREST`: `"light_forest"`
- `HEAVY_FOREST`: `"heavy_forest"`
- `WATER`: `"water"`
- `ICE`: `"ice"`
- `BRIDGE`: `"bridge"`
- `ROUGH`: `"rough"`
- `MOUNTAINS`: `"mountains"`
- `FIRE`: `"fire"`
- `SMOKE`: `"smoke"`
- `SNOW`: `"snow"`
- `BUILDING`: `"building"`
- `WALL`: `"wall"`
- `SAND`: `"sand"`

## GroundName

Alias: `"clear"|"road"|"rough"|"mountains"|"snow"|"sand"`

## GroundTypes

- `CLEAR`: `"clear"`
- `ROAD`: `"road"`
- `ROUGH`: `"rough"`
- `MOUNTAINS`: `"mountains"`
- `SNOW`: `"snow"`
- `SAND`: `"sand"`

## WoodsName

Alias: `"light"|"heavy"`

## WoodsTypes

- `LIGHT`: `"light"`
- `HEAVY`: `"heavy"`

## StructureKind

Alias: `"building"|"wall"|"bridge"`

## StructureKinds

- `BUILDING`: `"building"`
- `WALL`: `"wall"`
- `BRIDGE`: `"bridge"`

## LineOfSight

Alias: `"none"|"blocked"|"clear"`

## Placement

Alias: `{x: integer, y: integer, z?: integer}`

## MapUnitFilter

- `origin`: `HexCoordinate` — Filter anchor.
- `range`: `number` — Nonnegative hex radius.

## PerceptionReport

- `light`: `"night"|"twilight"|"day"` — Current battlefield light.
- `sight_range`: `integer` — Weather visibility in hexes, capped by the map ceiling.
- `lit_sight_range`: `integer` — Reach to illuminated targets; triple sight at night.
- `sensor_range`: `integer` — Effective all-conditions sensor band; zero while unavailable.
- `sensors`: `PerceptionStatus` — Condition of the sensor band.
- `probe`: `{kind: ProbeKind, range: integer, status: PerceptionStatus}|nil` — Best installed active probe.
- `radar`: `{range: integer, status: PerceptionStatus}|nil` — Anti-aircraft radar, if installed.
- `running`: `boolean` — Stopped units perceive nothing.
- `text`: `string` — The report printed by the sensor command.

## ProbeKind

Alias: `"beagle"|"light"|"bloodhound"|"watchdog"`

## ContactArc

Alias: `"front" | "right" | "rear" | "left"`

## ContactView

- `label`: `string` — Battlefield label, lowercase for identified allies.
- `coordinate`: `HexCoordinate`
- `elevation`: `integer` — Current elevation.
- `short_text`: `string` — Plain compact biped contact row.
- `verbose_text`: `string` — Plain multiline C0 contact report.
- `identified`: `boolean` — Current terrain permits identification.
- `weapon_arc`: `ContactArc` — Observer torso direction; individual weapons may have different arcs.
- `detection`: `DetectionChannel|nil` — How the observer currently perceives this contact; nil for clairvoyant-only views.
- `status`: `string` — Five visible condition columns; blank behind blocking terrain.
- `target`: `integer` — Acquired unit dbref.
- `name`: `string` — Chassis name, or "something" for unidentified signals.
- `friendly`: `boolean` — Identified and on the same team as observer.
- `range`: `Range`
- `network_range`: `number|nil` — Closest usable command-network sighting distance; nil without an active network.
- `heading`: `number` — Travel axis including lateral offset; reverse speed travels opposite this axis.
- `speed`: `number` — Current kph.

## SpotterEvents

- `events`: `SpotterEvent[]` — Independent requests in insertion order; detached inspection only.

## SpotterEvent

- `order`: `integer` — Global order among active events.
- `remaining`: `integer` — Seconds until connection completion or maintenance.
- `observer`: `integer` — Observer unit dbref.
- `positions`: `Point[]?` — Captured shooter and observer coordinates during setup; nil for maintenance.

## TagState

- `target`: `integer?` — Selected target; nil during recycle.
- `remaining`: `integer` — Lock/recycle seconds, zero through thirty.

## WeaponReadiness

- `weapon`: `string` — Conventional weapon kind.
- `intact`: `boolean`
- `ammunition`: `integer` — Matching available salvos.
- `recycle_remaining`: `integer` — Simulation seconds.
- `jammed`: `boolean` — Ammunition-feed failure blocks firing and mode changes.
- `spent`: `boolean` — Self-contained salvo has already launched.
- `posture_ready`: `boolean` — Prone support and mounting restrictions.
- `ready`: `boolean` — Power, mechanical conditions, preparation and supply permit use; targeting and authority remain separate.

## VehicleSectionName

Alias: `"front"|"right"|"left"|"rear"|"turret"|"rotor"`

## WeaponInspection

- `preferred_ammunition_section`: `string|nil` — Canonical preferred ammunition section; fallback remains automatic.
- `index`: `integer` — Zero-based stable weapon number.
- `name`: `string` — Equipment display name.
- `section`: `MechSectionName|VehicleSectionName`
- `failure`: `"jammed"|"shorted"|"dud"|"empty"|"disabled"|"ammunition_jam"|"critical_ammunition_jam"|nil` — Temporary operational failure independent of physical integrity.
- `rear_mount`: `boolean`
- `one_shot`: `boolean`
- `readiness`: `WeaponReadiness`
- `ammunition_mode`: `AmmunitionMode`
- `fire_mode`: `FireMode`

## AimModifiers

- `self_target`: `boolean` — Coolant self-application bypasses contact acquisition.
- `indirect`: `{spotter: integer, spotting: integer, movement: integer, target_lock: integer}|nil` — Observer contributions; perception then describes the spotter's view.
- `gunnery`: `integer`
- `distance`: `number`
- `network_range`: `{kind: "c3"|"c3i", distance: number, source: integer|nil}|nil` — Active command-network range; physical limits and firing visibility remain separate.
- `range`: `{bracket: string, modifier: integer}|nil`
- `attacker_movement`: `integer`
- `attacker_water`: `integer` — Plus one when firing at a unit from below the water surface.
- `woods_cover`: `integer` — Configured occupied-forest accuracy credit: zero, minus one or minus two.
- `target_movement`: `integer` — Movement contribution, including +1 for a VTOL with nonzero horizontal or vertical speed.
- `dug_in`: `integer` — Configured cover modifier, shared by Mech and vehicle attackers.
- `orbital_drop`: `integer` — Minus two while the target has an intact cocoon; zero after a breach.
- `heat`: `integer`
- `sensors`: `integer`
- `control_damage`: `integer` — Vehicle commander/sensor critical penalties.
- `mounting_section`: `integer`
- `targeting_computer`: `integer` — Eligible computer fire: -1 normally, +3 for a selected section on a mobile target.
- `aimed_section`: `integer` — Head aim penalty: 7 against immobile Mechs, 25 against mobile Mechs; overrides computer assistance.
- `beacon_accuracy`: `integer` — Haywire interference and iNarc homing assistance.
- `ammunition_accuracy`: `integer` — Selected ammunition adjustment; LB-X cluster is -3 versus VTOLs, otherwise -1; Stinger is -3 versus flying VTOLs and -1 during orbital descent.
- `targeting_mode`: `integer` — Scenario tracking-mode adjustment, separate from installed computer equipment.
- `weapon_accuracy`: `integer` — Intrinsic accuracy adjustment; pulse lasers contribute -2, MRMs +1.
- `weapon_damage`: `integer` — Penalty from damaged focusing, ranging and other weapon components.
- `target_lock`: `integer`
- `perception`: `{channel: DetectionChannel|nil, direct_fire: boolean, modifier: integer}|nil` — Nil without a current contact; direct_fire is false behind blocking terrain.

## SectionExposureReport

- `cause`: `"water"|"vacuum"`
- `section`: `MechSectionName`
- `reactor_explosion`: `table|nil`
- `fall`: `table|nil`
- `notices`: `{unit: integer, text: string}[]`

## TacticalImpact

- `impact`: `table` — Ordered material damage, critical losses and exposures (SectionExposureReport[]).
- `pilot_injuries`: `table[]` — Applied crew consequences.
- `notices`: `table[]` — Cockpit messages.
- `balance`: `table[]` — Applied balance checks and falls.
- `flooding`: `table[]` — Applied flooding consequences.

## AmmunitionDraw

- `bin_index`: `integer` — Zero-based bin index.
- `rounds`: `integer`

## WeaponUse

- `damage_penalty`: `integer` — Energy damage lost to focusing damage.
- `critical_failure`: `"barrel"|"crystal"|"feed"|nil` — Component responsible for a failed launch.
- `weapon`: `string`
- `ammunition`: `AmmunitionDraw[]` — Actual live-bin expenditure.
- `fire_mode`: `FireMode` — Effective mode after supply fallback.
- `heat`: `integer` — Already applied; do not add this heat again.
- `gatling_damage`: `integer|nil` — Supply-limited gatling damage before glancing.
- `ammunition_mode`: `AmmunitionMode`

## SalvoGroup

- `damage`: `integer`
- `hit`: `{section: MechSectionName, rear_armor: boolean, through_armor_critical: boolean, crew_stun: boolean}`
- `impact`: `table` — Ordered material phases, critical losses, exposures (SectionExposureReport[]), dump_ignitions, plasma_heat rolls, searchlight_destroyed and remaining scenario effects.
- `pilot_injuries`: `table[]` — Applied tactical injuries and consciousness results.
- `notices`: `{unit: integer, text: string}[]` — Already staged by unit.fire.
- `balance`: `table[]` — Applied balance checks and any nested falls.
- `flooding`: `table[]` — Applied section flooding and any nested falls.

## InfernoHit

- `target`: `integer`
- `missiles`: `integer` — Surviving missiles after interception.
- `burn_seconds`: `integer` — Duration added before immersion.
- `extinguished`: `boolean`
- `notices`: `Notice[]`

## WoodlandImpact

- `map`: `integer`
- `coordinate`: `HexCoordinate`
- `effect`: `{effect: "none"}|{effect: "ignite", seconds: integer}|{effect: "clear", clearing: "thin_to_light"|"cut_to_clear"|"cut_to_rough"}` — What the attack did to the woods.
- `notices`: `Notice[]`

## WoodsAbsorption

- `damage_before`: `integer` — Damage supplied to terrain before absorption: per shell for direct/burst fire, total for missiles after glancing cluster adjustment and interception.
- `damage_after`: `integer` — Remaining armor damage: minimum one per shell before glancing for direct/burst hits; whole-projectile totals may be zero.
- `terrain`: `WoodlandImpact` — Committed ignition or clearing check.
- `notices`: `Notice[]` — Ordered absorption and terrain feedback.

## MechSalvoReport

- `initial_woods`: `WoodsAbsorption|nil` — Nominal LBX terrain check before pellet counting and absorption.
- `woods`: `WoodsAbsorption|nil` — Occupied-woods consequences for direct shells (including bursts) or missile/pellet armor damage, after missile interception.
- `missiles_before_defense`: `integer|nil` — Cluster hits before automatic defenses.
- `cluster_roll`: `integer|nil` — Original missile cluster roll; nil for direct non-missile hits.
- `inferno`: `InfernoHit|nil` — Burning replaces armor damage.
- `groups`: `SalvoGroup[]`

## CharacterValue

- `value`: `integer` — Trained skill level.
- `experience`: `integer` — Encoded earned levels and XP balance.
- `last_used`: `integer` — Last accepted award timestamp.

## ExperienceAward

- `accepted`: `boolean`
- `before`: `CharacterValue`
- `after`: `CharacterValue`

## PilotingCheck

- `skill`: `integer` — Base pilot skill target.
- `damage`: `integer` — Penalty from physical damage.
- `cockpit`: `integer` — Small cockpit construction penalty, independent of damage.
- `situational`: `integer` — Caller-supplied modifier.
- `absent_character_pilot`: `integer` — Penalty for an absent in-character pilot.
- `target`: `integer` — Total required roll.
- `roll`: `integer|nil` — No dice when already prone or unable to act.
- `success`: `boolean`
- `experience`: `ExperienceAward|nil` — Accepted or rate-limited skill mutation for XP-awarding callers.

## MechFallReport

- `avoidance`: `PilotingCheck|nil` — Personal-injury check; combat-safe units skip it.
- `pilot`: `integer|nil` — Pilot dbref captured before fall injuries can clear the assignment.
- `experience_messages`: `DiagnosticMessage[]` — Accepted protection-check diagnostics captured before fall damage.
- `pilot_injury`: `table|nil` — Tactical crew injury.
- `character_injury`: `table|nil` — Personal injury to an assigned character pilot, independent of tactical crew health.
- `direction_roll`: `integer`
- `arc`: `"Front"|"Rear"|"Left"|"Right"`
- `damage`: `integer`
- `groups`: `SalvoGroup[]` — Fall damage in the order it was applied.
- `mines`: `table` — Mine activation after the fall damage sequence.
- `ice_break`: `SurfaceBreak|nil` — Ice fracture can cause nested water falls for this unit and its neighbors.
- `flooding`: `SectionExposureReport[]`
- `inferno_notices`: `Notice[]`

## RecoilReport

- `experience_messages`: `DiagnosticMessage[]` — Accepted recoil XP diagnostics published with the shot.
- `check`: `PilotingCheck`
- `fall`: `MechFallReport|nil`

## AmsReport

- `weapon_index`: `integer` — Zero-based defensive weapon index.
- `ammunition_bin`: `integer|nil` — Selected normal-ammunition bin; nil for laser AMS, which uses none.
- `roll`: `integer` — Interception capacity before rack and cluster limits.
- `ammunition_spent`: `integer` — May be less than interception capacity.
- `shot_down`: `integer` — Actual intercepted hits after the missile meets its base target number.

## BeaconKind

Alias: `"narc"|"homing"|"haywire"|"ecm"`

## NarcReport

- `kind`: `BeaconKind`
- `notices`: `Notice[]` — Cockpit effects from the hit-location roll.
- `hit`: `boolean` — Whether the beacon met the full attack target.
- `intercepted`: `boolean` — Whether AMS intercepted the pod.
- `section`: `MechSection|VehicleSectionName|nil` — Surviving attachment section.
- `rear`: `boolean` — Rear-facing attachment notice.

## MechShotReport

- `launch_notices`: `Notice[]` — Cocoon opening feedback before target consequences.
- `coordinate`: `{x: integer, y: integer}|nil` — Coordinate-directed shot; target identifies the selected occupant.
- `experience_messages`: `table[]` — Accepted spotting/artillery awards, including misses.
- `streak_confused`: `boolean` — Angel interference disables Streak homing.
- `narc`: `NarcReport|nil` — Normal beacon outcome; explosive pods use salvo damage.
- `ams`: `AmsReport|nil` — Automatic defense activation; absent for missile rolls below base target number.
- `ammunition_warning`: `string|nil` — Pre-expenditure warning staged with the shot.
- `shooter`: `integer`
- `target`: `integer`
- `weapon_index`: `integer` — Zero-based stable weapon number.
- `aim`: `AimModifiers`
- `target_number`: `integer|nil` — Ordinary aim subtotal; nil beyond physical range.
- `roll`: `integer`
- `glancing`: `boolean`
- `recoil`: `RecoilReport|nil` — Moving Heavy Gauss control check and fall.
- `jammed`: `boolean` — Recoverable ammunition-feed failure without expenditure.
- `loader_destroyed`: `boolean` — Permanent mount loss from loader failure or propellant ignition.
- `propellant_roll`: `integer|nil` — Second caseless roll; eight or more ignites propellant.
- `misload`: `TacticalImpact|nil` — Applied misload or propellant ignition damage.
- `launched`: `boolean` — False for failed Streak lock: no heat/ammo expenditure, but weapon recycles.
- `hit`: `boolean` — Launch classification; missile near misses may have no target effects.
- `expenditure`: `WeaponUse`
- `salvo`: `{kind: 'mech'|'vehicle'|'swarm', report: table}|nil` — Target-specific damage; nil on a miss or a heat-mode hit.
- `heat_transfer`: `integer` — Heat already added to the target, zero unless a heat-mode shot hits.
- `thermal_woods`: `WoodsAbsorption|nil` — Terrain effects and feedback preceding thermal transfer; heat/cooling strength remains unchanged.
- `missed_terrain`: `WoodlandImpact|nil` — Incidental terrain check after a launched non-missile miss, independent of woods damage configuration.
- `cooling`: `number?` — Coolant reduction applied to stored heat, including temporary negative credit.

## VehicleShotReport

- `experience_messages`: `DiagnosticMessage[]` — Accepted spotting/artillery awards, including misses.
- `coordinate`: `{x: integer, y: integer}|nil` — Occupied-hex shot; target identifies the selected occupant.
- `shooter`: `integer`
- `target`: `integer`
- `weapon_index`: `integer` — Zero-based stable weapon number.
- `aim`: `AimModifiers`
- `streak_confused`: `boolean`
- `ammunition_warning`: `string|nil` — Pre-expenditure warning staged with the shot.
- `launch_notices`: `Notice[]` — Cocoon opening feedback before target consequences.
- `target_number`: `integer|nil` — Ordinary aim subtotal; nil beyond physical range.
- `roll`: `integer`
- `launched`: `boolean` — False for failed Streak lock: no heat/ammo expenditure, but weapon recycles.
- `hit`: `boolean` — Launch classification; missile near misses may have no target effects.
- `glancing`: `boolean` — Tactical missile shots use the base target-number boundary.
- `recoil`: `RecoilReport|nil` — Always nil: vehicles make no recoil control check.
- `jammed`: `boolean` — Recoverable ammunition-feed failure without expenditure.
- `loader_destroyed`: `boolean` — Permanent mount loss from loader failure or propellant ignition.
- `propellant_roll`: `integer|nil` — Second caseless roll; eight or more ignites propellant.
- `misload`: `table|nil` — Shooter internal damage and critical consequences.
- `expenditure`: `VehicleWeaponUse`
- `ams`: `AmsReport|nil`
- `narc`: `NarcReport|nil` — Beacon attachment or interception; vehicle sections use their own names.
- `cooling`: `number|nil` — Coolant removed from target stored heat.
- `heat_transfer`: `integer` — Direct flamer heat added to the target, otherwise zero.
- `thermal_woods`: `WoodsAbsorption|nil` — Terrain effects and feedback preceding thermal transfer; heat/cooling strength remains unchanged.
- `missed_terrain`: `WoodlandImpact|nil` — Incidental terrain check after a launched non-missile miss, independent of woods damage configuration.
- `salvo`: `{kind: 'mech'|'vehicle'|'swarm', report: table}|nil` — Target-specific ordered damage groups.

## VehicleInfernoHit

- `missiles`: `integer` — Surviving missiles after clustering and interception.
- `explosion_roll`: `integer|nil` — Standard mobile-vehicle heat check.
- `burn_seconds`: `integer` — Jelly duration added to a stationary unit.
- `damage`: `table[]` — Ordered initial section fire damage.
- `explosion`: `table|nil` — Completed heat catastrophe.
- `notices`: `Notice[]` — Already staged by firing.
- `broadcasts`: `Notice[]` — Raw damage broadcasts handled by firing.

## VehicleSalvoReport

- `initial_woods`: `WoodsAbsorption|nil` — Nominal LBX terrain check before pellet counting and absorption.
- `woods`: `WoodsAbsorption|nil` — Occupied-woods consequences for direct shells (including bursts) or missile/pellet armor damage, after missile interception.
- `experience`: `table[]` — Per-packet optional pre-impact XP awards.
- `experience_messages`: `table[]` — Ordered XP channel diagnostics.
- `cluster_roll`: `integer|nil`
- `missiles_before_defense`: `integer|nil`
- `groups`: `table[]` — Located conventional damage packets.
- `inferno`: `VehicleInfernoHit|nil` — Dedicated vehicle inferno outcome.

## VehicleWeaponUse

- `weapon`: `string`
- `ammunition`: `AmmunitionDraw[]` — Actual live-bin expenditure.
- `ammunition_mode`: `AmmunitionMode`
- `fire_mode`: `FireMode` — Effective mode after supply fallback.
- `gatling_damage`: `integer|nil` — Supply-limited gatling damage before glancing.
- `launched`: `boolean` — False for a failed Streak lock; also reported on the shot.
- `heat`: `integer` — Already applied; do not add this heat again.

## ArtilleryAim

- `target_number`: `integer` — Out-of-range and underwater aims keep the reference's impossible target numbers.
- `maximum_range`: `integer`
- `range`: `"in_range"|"out_of_range"|"underwater"`

## VehicleInternalDamage

- `section`: `VehicleSectionName`
- `incoming`: `integer`
- `structural_damage`: `integer`
- `rolls`: `integer[]` — Internal critical roll, preceded by a damage-entry roll for standalone explosions.
- `absorbed`: `integer`
- `discarded`: `integer` — Vehicle-local internal explosions do not transfer excess to another section.
- `destroyed_sections`: `VehicleSectionName[]`
- `unit_destroyed`: `boolean`
- `criticals`: `table[]` — Critical resolutions in execution order.
- `notices`: `Notice[]` — Includes nested critical notices in execution order.
- `pilot_notices`: `table[]` — Pilot-only control feedback indexed into the damage notice stream.
- `broadcasts`: `Notice[]`

## ArtilleryLaunchReport

- `launch_notices`: `Notice[]` — Cocoon opening feedback before target consequences.
- `shooter`: `integer`
- `map`: `integer`
- `coordinate`: `{x: integer, y: integer}`
- `weapon_index`: `integer`
- `aim`: `ArtilleryAim`
- `roll`: `integer`
- `hit`: `boolean`
- `launched`: `boolean`
- `jammed`: `boolean`
- `loader_destroyed`: `boolean`
- `propellant_roll`: `integer|nil`
- `expenditure`: `WeaponUse`
- `misload`: `{kind: "mech"|"vehicle", report: TacticalImpact|VehicleInternalDamage}|nil`
- `ammunition_warning`: `string|nil`
- `queued_shot`: `integer|nil` — Persistent map queue ordinal, present after launch.

## HexShotReport

- `launch_notices`: `Notice[]` — Cocoon opening feedback before target consequences.
- `shooter`: `integer`
- `map`: `integer`
- `coordinate`: `{x: integer, y: integer}`
- `weapon_index`: `integer`
- `aim`: `HexAimModifiers`
- `target_number`: `integer|nil`
- `roll`: `integer`
- `hit`: `boolean`
- `launched`: `boolean`
- `jammed`: `boolean`
- `loader_destroyed`: `boolean`
- `propellant_roll`: `integer|nil`
- `expenditure`: `WeaponUse`
- `misload`: `{kind: "mech"|"vehicle", report: TacticalImpact|VehicleInternalDamage}|nil`
- `ammunition_warning`: `string|nil`
- `cluster_roll`: `integer|nil`
- `terrain`: `table[]` — Applied woodland effects and captured notices.
- `surfaces`: `table[]` — Structural rolls, optional fracture/falls, and notices.
- `buildings`: `table[]` — Building identity, actual damage, remaining integrity and notices.
- `recoil`: `RecoilReport|nil`

## AimSelection

Alias: `{class: "mech", section: string}|{class: "ground_vehicle"|"vtol", section: string}`

## SightReport

- `shooter`: `integer`
- `weapon_index`: `integer`
- `weapon`: `string`
- `target`: `integer|nil`
- `coordinate`: `HexCoordinate|nil`
- `aim`: `AimModifiers|HexAimModifiers|ArtilleryAim`
- `target_number`: `integer|nil` — Nil when out of range.
- `roll`: `integer` — Attack dice consumed without launching.
- `gatling_roll`: `integer|nil` — Preparation intensity, without an ammunition cap.
- `partial_cover`: `boolean`

## WeaponValues

- `recycle_seconds`: `integer` — Effective runtime recycle time, from 1 through 127 seconds.
- `battle_value`: `integer` — Effective runtime Battle Value, from 0 through 2147483647.

## InventoryEntry

- `part_id`: `integer` — Stable game-directory part identifier.
- `quantity`: `integer` — Positive stock quantity, at most 2147483647.

## Part

- `part_id`: `integer` — Stable inventory identifier.
- `name`: `string` — Canonical stock name.
- `kind`: `"weapon"|"ammunition"|"component"|"commodity"|"bomb"`
- `mass`: `integer` — Catalogue mass in 1/1024 tons; loose bomb stock uses four times this value.

## InventoryCleanup

- `original_entries`: `integer`
- `new_entries`: `integer`
- `items`: `integer`

## CargoRow

- `name`: `string` — Stock display name.

## BattleTechInspection

- `database`: `btech.database` — Explicit world checkpoints.
- `cargo`: `table` — Cockpit stock reports and transfers.
- `inventory`: `table` — Shared loose-parts stock.
- `weapon`: `table` — Runtime weapon settings.
- `character`: `table`
- `template`: `table`
- `map`: `table`
- `player`: `table` — Saved player preferences.
- `unit`: `table`
- `parts`: `table` — Registered part catalogue and stock queries.
- `repair`: `table` — Immediate repair requests and technician scheduling.
- `system`: `table` — World event telemetry.
- `autopilot`: `BtechAutopilotAPI` — Lua control of unit-attached ground autopilots.
- `tactical`: `BtechTacticalAPI` — Filtered group observations and atomic intentions.
- `errors`: `table` — Structured btech error-code tree from mux.error.code_tree('btech').

## AmmunitionAdjustment

- `location`: `CriticalLocation`
- `supplied`: `integer` — Authored initial quantity.
- `normalized`: `integer` — Initial quantity after construction normalization.
- `inferred_half_ton`: `boolean` — Construction would infer a half-ton bin.

## TemplateCheck

- `chassis`: `"biped"|"quad"|nil` — Parsed anatomy, independent of simulation readiness.
- `name`: `string`
- `reference`: `string`
- `constructible`: `boolean` — Passes currently implemented biped construction checks.
- `rejection`: `string|nil` — First construction failure; nil for a constructible template.
- `weapons`: `integer` — Resolved weapon count on success; zero on rejection.
- `ammunition_bins`: `integer` — Resolved bin count on success; zero on rejection.
- `ammunition_adjustments`: `AmmunitionAdjustment[]` — Changes on successful construction.

## ArtemisController

- `location`: `{section: MechSectionName|VehicleSectionName, slot: integer}` — Zero-based controller position.
- `link`: `integer` — One-based template launcher slot; zero is unassigned.
- `weapon_indices`: `integer[]` — Zero-based missile mounts matching the link.
- `operational`: `boolean` — Controller is available under the unit’s equipment damage rules.

## Unjam

- `weapon_index`: `integer` — Zero-based weapon number.
- `remaining`: `integer` — Committed seconds remaining, 1 through 60.

## EquipmentCondition

Alias: `'empty'|'operational'|'damaged'|'disabled'|'broken'|'destroyed'|'jammed'|'shorted'|'ammo_jam'`

## WeaponDamageEffects

- `moderate`: `integer` — General accuracy penalty.
- `ranging`: `integer` — Accuracy penalty beyond short range.
- `heat`: `integer` — Additional firing heat.
- `damage`: `integer` — Energy damage reduction.
- `explosion`: `integer` — Nonzero count explodes on an attack roll of count plus one or less.
- `jam`: `integer` — Nonzero count jams on an attack roll of count plus one or less.
- `feed_locked`: `boolean` — Prevents changing ammunition modes.

## WeaponDiagnostic

- `index`: `integer` — Zero-based installed mount number, including destroyed mounts.
- `weapon`: `string` — Catalogue weapon identifier (snake case).
- `section`: `string` — Chassis-specific location name.
- `condition`: `EquipmentCondition`
- `damaged_slots`: `integer`
- `destroyed_slots`: `integer`
- `disabled_slots`: `integer`
- `effects`: `WeaponDamageEffects` — Existing firing penalties, without recomputation in Lua.
- `preferred_ammunition_section`: `string?`

## WeaponSpecification

- `weapon`: `Weapon` — Catalogue weapon identifier (snake case).
- `ammunition`: `AmmunitionMode` — MMLs have separate normal (SRM) and mml_lrm rows.
- `heat`: `integer`
- `damage`: `integer`
- `minimum_range`: `integer`
- `short_range`: `integer`
- `medium_range`: `integer`
- `long_range`: `integer` — Effective range in hexes, including artillery map-sheet conversion.
- `extended_range`: `integer?` — Present when extended range is configured.
- `recycle_seconds`: `integer` — Effective runtime value for new activations.

## CriticalInspection

- `slot`: `integer` — Zero-based physical slot; native labels add one.
- `equipment`: `string` — Resolved display name, including bin mode.
- `condition`: `EquipmentCondition`
- `weapon_index`: `integer?` — Stable zero-based mount index, including split extensions.
- `ammunition_index`: `integer?` — Zero-based bin index.
- `ammunition_remaining`: `integer?` — Saved bin quantity; native text hides it when unavailable.
- `ammunition_capacity`: `integer?` — Installed bin capacity, including special rounds and half tons.
- `rear_mount`: `boolean`
- `one_shot`: `boolean`
- `spent`: `boolean`
- `controls_slot`: `integer?` — Authored Artemis display label, already one-based.

## CriticalReport

- `section`: `string` — Stable Mech or vehicle section identity.
- `name`: `string` — Chassis-specific display heading.
- `slots`: `CriticalInspection[]` — All six or twelve physical slots, including empty ones.

## ElectronicMode

Alias: `"off"|"ecm"|"eccm"`

## ElectronicField

- `protected`: `boolean`
- `angel_protected`: `boolean`
- `disturbed`: `boolean`
- `angel_disturbed`: `boolean`
- `countered`: `boolean`

## Electronics

- `guardian`: `ElectronicMode`
- `angel`: `ElectronicMode`
- `field`: `ElectronicField`

## PodRow

- `section`: `MechSection|VehicleSectionName`
- `destroyed`: `boolean`
- `kinds`: `BeaconKind[]`

## PodRemoval

- `section`: `MechSection`
- `kind`: `BeaconKind`
- `arm`: `"left"|"right"`
- `target_number`: `integer`
- `roll`: `integer`
- `removed`: `boolean`
- `self_damage`: `integer`
- `impact`: `TacticalImpact|nil`
- `notices`: `Notice[]`

## SignatureTransition

- `enabled`: `boolean`
- `remaining`: `integer`

## SignatureState

- `enabled`: `boolean`
- `pending`: `SignatureTransition|nil`

## HexLock

- `hex`: `{x: integer, y: integer}`
- `mode`: `'unit_at_hex'|'hex'|'building'|'ignite'|'clear'`
- `remaining`: `integer` — Eight seconds to settle; zero is settled.

## HexAimModifiers

- `hex`: `{x: integer, y: integer}`
- `mode`: `'unit_at_hex'|'hex'|'building'|'ignite'|'clear'`
- `visible`: `boolean` — Current terrain visibility, separate from numeric aim.
- `hex_bonus`: `integer` — Zero for unit-at-hex, otherwise -4.
- `subtotal`: `integer|nil` — Nil beyond weapon range; numeric aim alone does not authorize firing.

## RadioReception

- `receiver`: `integer`
- `channel`: `integer` — Zero-based receiving channel.
- `transmitters`: `integer[]` — Sender and any relays, excluding receiver.
- `bearing`: `integer` — Bearing toward final transmitter.
- `text`: `string` — Formatted cockpit message.

## DiagnosticMessage

- `channel`: `"debug"|"economy"|"attack_experience"|"experience"|"piloting_experience"|"frequencies"|"zero_frequencies"|"map_errors"`
- `text`: `string`

## RadioTransmission

- `delivery`: `{mode: 'analog'|'digital', report: {sender: integer, map: integer, frequency: integer, receptions: RadioReception[], interfered_receivers: integer[]?, scans: {receiver: integer, channel: integer, previous: integer, frequency: integer}[]?, notifications: Notice[]?}}`
- `mines`: `table` — Ordered frequency-matched command-mine report and consequences.
- `audit_messages`: `DiagnosticMessage[]` — Diagnostics committed with the transmission.
- `experience_messages`: `DiagnosticMessage[]` — Accepted communication XP diagnostics.

## TargetedRadioReport

- `sender`: `integer`
- `target`: `integer`
- `notices`: `{unit: integer, text: string}[]` — Captured sender echo and powered-recipient message.

## BuildingScan

- `text`: `string` — Cockpit reply; undiscovered and missing buildings share one message.
- `experience_messages`: `DiagnosticMessage[]` — Accepted perception diagnostics.

## MineScan

- `found`: `boolean` — Successful recognition only; configuration is never disclosed.
- `text`: `string`
- `experience_messages`: `DiagnosticMessage[]`

## HexScan

- `building`: `BuildingScan`
- `mines`: `MineScan`

## SelectedScan

- `kind`: `'unit'|'building'|'hex'`
- `report`: `string|BuildingScan|HexScan`

## ViewPosition

- `map`: `integer` — Scanner battlefield dbref.
- `center`: `{x: integer, y: integer}` — Requested center before viewport clipping.
- `maximum_range`: `integer` — Damage-adjusted display hardware radius.

## ViewDimensions

- `tactical_width`: `integer?` — Requested columns, 5..40; default 21.
- `tactical_height`: `integer?` — Requested rows, 5..24; default 14.
- `long_range_height`: `integer?` — Requested rows, 10..40; default 11.

## Viewport

- `map`: `integer`
- `requested_center`: `{x: integer, y: integer}`
- `origin`: `{x: integer, y: integer}` — Upper-left in-bounds coordinate.
- `width`: `integer` — Column count.
- `height`: `integer` — Row count.
- `maximum_range`: `integer`

## LongRangeMap

- `viewport`: `Viewport`
- `text`: `string` — Filtered staggered-row display.

## TacticalMap

- `viewport`: `Viewport`
- `text`: `string` — Styled hex display with acquired two-character contact labels.

## HexCenterReport

- `coordinate`: `HexCoordinate`
- `elevation`: `integer`
- `range`: `number` — Horizontal range to the current hex center.
- `bearing`: `integer` — Clockwise degrees; 180 at the exact center.
- `text`: `string` — Shared native readout.

## NavigationReport

- `center`: `HexCoordinate` — Requested local map center.
- `text`: `string` — Styled local map, continuous-position plot and live readouts.

## BuildingContactMode

Alias: `"follow_brief" | "include" | "exclude"`

## ContactPreferences

- `include_dead`: `boolean` — Defaults false.
- `include_shutdown`: `boolean` — Defaults true.
- `include_enemies`: `boolean` — Defaults true.
- `include_allies`: `boolean` — Defaults true.
- `include_target`: `boolean` — Defaults true; never bypasses visibility.
- `buildings`: `BuildingContactMode` — Defaults "exclude"; applies to native contacts +, independently of unit filtering.

## ContactOptions

- `buildings`: `boolean` — Include building contacts for native output.
- `preferences`: `ContactPreferences` — Decoded unit categories.
- `ignored`: `string[]` — Unrecognized characters in encounter order.

## BuildingContact

- `detection`: `DetectionChannel|nil` — Whether the sensor band or sight reaches the entrance.
- `short_text`: `string` — Plain compact row after identification locks.
- `weapon_arc`: `ContactArc` — Observer torso direction toward entrance.
- `interior`: `integer`
- `coordinate`: `HexCoordinate`
- `elevation`: `integer`
- `name`: `string` — Plain structure name.
- `range`: `number`
- `bearing`: `integer`
- `integrity`: `integer`
- `maximum_integrity`: `integer`
- `identified`: `boolean` — Identification lock result.
- `hidden`: `boolean` — Concealed entrance identified successfully.
- `status`: `string` — Blank, x (restricted), X (safe/restricted command center), or C (command center).

## BriefSettings

- `contacts`: `integer` — Contact mode 0..3; defaults 1.
- `automatic`: `integer` — Routine notice mode 0..6; defaults 0.

## BriefReport

- `settings`: `BriefSettings`
- `changed`: `boolean` — An edit was requested; query is false.
- `text`: `string` — Query or cockpit confirmation.

## BootleggerReport

- `modifier`: `integer` — Situational difficulty and failed-fall severity.
- `check`: `PilotingCheck`
- `fall`: `MechFallReport?`
- `notices`: `Notice[]`

## EtaReport

- `coordinate`: `HexCoordinate`
- `range`: `number` — Horizontal range.
- `minutes`: `integer?` — Whole minutes, absent when effectively stationary.
- `text`: `string`

## BearingReport

- `origin`: `Point`
- `destination`: `Point`
- `bearing`: `integer` — Clockwise compass degrees, 180 for coincident points.
- `text`: `string`

## RangeReport

- `horizontal`: `number` — Horizontal distance in hexes.
- `spatial`: `number` — Spatial distance after dark-map terrain masking.
- `text`: `string`

## VectorReport

- `horizontal`: `number`
- `spatial`: `number`
- `bearing`: `integer` — Clockwise compass bearing.
- `vertical_bearing`: `integer` — Signed vertical angle rounded away from zero.
- `text`: `string`

## BoosterState

- `enabled`: `boolean`
- `counter`: `integer`
- `remaining`: `integer` — Seconds until the next overload/recovery check.
- `failed`: `boolean` — Hardware failure persists through shutdown.

## NetworkStatusRow

- `unit`: `integer`
- `label`: `string`
- `name`: `string`
- `coordinate`: `HexCoordinate`
- `elevation`: `integer`
- `range`: `number`
- `bearing`: `integer`
- `speed`: `number`
- `heading`: `integer`
- `armor_percent`: `integer`
- `internal_percent`: `integer`

## SwarmHop

- `target`: `integer`
- `incoming`: `integer`
- `roll`: `integer`
- `remaining`: `integer`
- `salvo`: `{kind: 'mech'|'vehicle', report: table}|nil` — Absent for a secondary miss.

## SwarmReport

- `launched`: `integer`
- `remaining`: `integer`
- `traveled`: `number` — Cumulative distance, including a terminal leg that falls short.
- `hops`: `SwarmHop[]` — Ordered attacks, at most eleven.
- `notices`: `Notice[]`
- `broadcasts`: `Notice[]`

## WeaponDamage

- `location`: `{section: MechSectionName, slot: integer}`
- `effects`: `("moderate"|"focus"|"crystal"|"ranging"|"barrel"|"feed")[]` — Distinct component effects; empty means superficial damage.

## RuntimeStats

- `simulation_pending`: `boolean` — Same work predicate as the server's one-second simulation tick.
- `scanner_observers`: `integer`
- `reactor_startup_remaining`: `integer`
- `artillery_shots`: `integer`
- `maps`: `integer`
- `mechs`: `integer`
- `vehicles`: `integer`
- `registration_kinds`: `table<string, integer>`
- `inline_record_bytes`: `integer` — Root/map/unit inline sizes only; heap storage excluded.
- `encoded_state_bytes`: `integer` — Exact compact JSON encoding size, not allocator usage.

## UnitField

- `name`: `string` — Full field name, independent of display width.
- `value`: `string|nil` — Available field value; nil displays as n/a.

## UnitFieldReport

- `unit`: `integer`
- `columns`: `integer`
- `fields`: `UnitField[]`
- `text`: `string` — Literal report text, already published to the actor.

## BattleValuePair

- `current`: `integer`
- `original`: `integer`

## ArmorStatus

- `section`: `MechSection` — Omitted when the request did not select one.
- `armor`: `BattleValuePair`
- `internal`: `BattleValuePair`
- `rear_armor`: `BattleValuePair`

## AmmunitionStatus

- `rounds`: `integer`
- `capacity`: `integer`

## WeaponStats

- `kind`: `string`
- `heat`: `integer`
- `damage`: `integer`
- `minimum_range`: `integer`
- `short_range`: `integer`
- `medium_range`: `integer`
- `long_range`: `integer`
- `critical_slots`: `integer`
- `ammunition_per_ton`: `integer`
- `recycle_time`: `integer`
- `battle_value`: `integer`

## PartDefinition

- `id`: `integer` — Stable catalogue part identifier.
- `short_name`: `string`
- `long_name`: `string`
- `very_long_name`: `string`
- `category`: `string`
- `weight_tons`: `number`
- `cost`: `integer`
- `weapon`: `WeaponStats` — Present for weapon parts.

## PartRef

Alias: `PartDefinition|integer|string`

## PartStack

- `part`: `PartDefinition`
- `quantity`: `integer`

## PartCategory

- `code`: `string`
- `name`: `string`

## CriticalSlot

- `section`: `MechSection`
- `slot`: `integer`
- `kind`: `string`
- `part`: `PartDefinition`
- `operational`: `boolean`
- `temporary_failure`: `boolean`
- `auxiliary_data`: `integer`
- `ammunition`: `AmmunitionStatus`
- `fire_modes`: `FireModeConstant[]`
- `ammunition_modes`: `AmmunitionModeConstant[]`

## MountedWeapon

- `number`: `integer` — Zero-based stable weapon number.
- `section`: `MechSection`
- `first_slot`: `integer` — Zero-based first occupied critical slot.
- `part`: `PartDefinition`
- `slot_count`: `integer`
- `recycle`: `integer` — Seconds remaining in the current cycle.
- `recycle_time`: `integer` — Full recycle time in seconds.
- `operational`: `boolean`

## Engine

- `rating`: `integer`
- `suspension_factor`: `integer`

## RadioChannelReport

- `channel`: `integer` — One-based channel position.
- `frequency`: `integer` — Frequency from 0 through 999999.
- `title`: `string` — At most fifteen UTF-8 bytes.
- `modes`: `string[]` — Active mode names: digital, mute, relay, information, scan.

## BattleValue

- `total`: `number`
- `offensive`: `number`
- `defensive`: `number`

## Technology

- `code`: `TechnologyCode`
- `name`: `string`
- `group`: `"primary"|"secondary"|"infantry"`
- `source`: `"configured"|"inferred"`

## CharacterValueDefinition

- `code`: `integer`
- `name`: `string`
- `kind`: `string` — Char_value, Char_skill, Char_advantage or Char_attribute.
- `default_experience_threshold`: `integer`

## CharacterValueReport

- `definition`: `CharacterValueDefinition`
- `amount`: `integer`
- `target`: `integer` — Skill targets including earned levels.
- `experience`: `integer`
- `experience_to_next_level`: `integer`

## PersonalCombatArmor

- `head`: `integer`
- `torso`: `integer`
- `hands`: `integer`
- `feet`: `integer`

## PersonalCombatEquipment

- `weapon`: `PartDefinition`
- `ammunition`: `integer`

## PersonalCombatLoadout

- `armor`: `PersonalCombatArmor`
- `right`: `PersonalCombatEquipment`
- `left`: `PersonalCombatEquipment`

## UiPreferencesState

- `tactical_height`: `integer`
- `tactical_width`: `integer`
- `lrs_height`: `integer`
- `include_dead`: `boolean`
- `include_shutdown`: `boolean`
- `include_enemies`: `boolean`
- `include_allies`: `boolean`
- `include_target`: `boolean`
- `buildings`: `"follow_brief"|"include"|"exclude"`
- `configured`: `boolean`

## WeaponInstall

- `part`: `PartRef` — Weapon part reference.
- `section`: `MechSection`
- `slots`: `integer[]` — Zero-based critical slots.
- `rear_facing`: `boolean`
- `targeting_computer`: `boolean`
- `one_shot`: `boolean`

## AmmunitionConfiguration

- `weapon`: `PartRef` — Launcher part reference.
- `section`: `MechSection`
- `slot`: `integer` — Zero-based critical slot.
- `half_ton`: `boolean`
- `ammunition_modes`: `AmmunitionModeConstant[]`

## WeaponModes

- `fire_modes`: `FireModeConstant[]`
- `ammunition_modes`: `AmmunitionModeConstant[]`

## SpecialInstall

- `part`: `PartRef` — Omit to empty the slot.
- `section`: `MechSection`
- `slot`: `integer` — Zero-based critical slot.
- `auxiliary_data`: `integer`

## RepairArmorRequest

- `operation`: `RepairOperation`
- `section`: `MechSection`
- `value`: `integer`

## RepairInternalRequest

- `operation`: `RepairOperation`
- `section`: `MechSection`
- `value`: `integer`

## RepairRearArmorRequest

- `operation`: `RepairOperation`
- `section`: `MechSection`
- `value`: `integer`

## RepairPartRequest

- `operation`: `RepairOperation`
- `section`: `MechSection`
- `slot`: `integer`

## RepairReattachRequest

- `operation`: `RepairOperation`
- `section`: `MechSection`

## ImmediateRepair

Alias: `RepairArmorRequest|RepairInternalRequest|RepairRearArmorRequest|RepairPartRequest|RepairReattachRequest`

## AutopilotStates

- `PAUSED`: `"paused"`
- `IDLE`: `"idle"`
- `EXECUTING`: `"executing"`
- `BLOCKED`: `"blocked"`

## AutopilotOrderStates

- `QUEUED`: `"queued"`
- `RUNNING`: `"running"`
- `SUCCEEDED`: `"succeeded"`
- `FAILED`: `"failed"`
- `CANCELED`: `"canceled"`

## AutopilotReasons

- `MANUAL_TAKEOVER`: `"manual_takeover"`
- `CONTACT_LOST`: `"contact_lost"`
- `STUCK`: `"stuck"`
- `UNREACHABLE`: `"unreachable"`
- `INVALIDATED`: `"invalidated"`
- `RESOURCE_LIMIT`: `"resource_limit"`
- `CONGESTED`: `"congested"`
- `INVALID_TARGET`: `"invalid_target"`
- `UNIT_UNAVAILABLE`: `"unit_unavailable"`
- `MAP_CHANGED`: `"map_changed"`
- `UNSUPPORTED`: `"unsupported"`
- `STALE_REVISION`: `"stale_revision"`

## AutopilotRangeBand

- `minimum`: `integer` — Inclusive minimum engagement range in hexes.
- `maximum`: `integer` — Inclusive maximum engagement range in hexes.

## AutopilotConfig

- `speed_percent`: `integer` — Desired speed as a percentage from 0 through 100.
- `fire_mode`: `"hold"|"assigned_target"|"opportunistic"` — Current serialized weapon policy.
- `heat_ceiling`: `integer` — Projected heat limit for autonomous fire.
- `preferred_range`: `AutopilotRangeBand|nil` — Optional engagement band.

## AutopilotConfigPatch

- `speed_percent`: `integer|nil` — Optional speed update.
- `fire_mode`: `AutopilotFireMode|nil` — Optional weapon-policy update.
- `heat_ceiling`: `integer|nil` — Optional projected heat limit.
- `preferred_range`: `AutopilotRangeBand|false|nil` — Set or clear the preferred band.

## AutopilotControllerState

Alias: `"paused"|"idle"|"executing"|"blocked"`

## AutopilotOrderState

Alias: `"queued"|"running"|"succeeded"|"failed"|"canceled"`

## AutopilotFeedbackEvent

Alias: `"configured"|"paused"|"resumed"|"manual_takeover"|"order_queued"|"order_started"|"order_succeeded"|"order_failed"|"order_canceled"|"blocked"`

## AutopilotReason

Alias: `"manual_takeover"|"contact_lost"|"stuck"|"unreachable"|"invalidated"|"resource_limit"|"congested"|"invalid_target"|"unit_unavailable"|"map_changed"|"unsupported"|"stale_revision"`

## AutopilotOrder

- `kind`: `AutopilotOrderName`
- `destination`: `Position|nil` — Move or attack-move destination.
- `arrival_radius`: `integer|nil` — Destination tolerance in hexes.
- `target`: `integer|nil` — Follow or attack target unit.
- `separation`: `integer|nil` — Follow distance in hexes.
- `waypoints`: `Position[]|nil` — Patrol route.
- `range`: `AutopilotRangeBand|nil` — Optional attack engagement band.

## AutopilotStoredOrder

- `kind`: `"move"|"hold"|"follow"|"patrol"|"attack"|"attack_move"` — Serialized intent kind.
- `destination`: `Position|nil` — Move or attack-move destination.
- `arrival_radius`: `integer|nil` — Destination tolerance in hexes.
- `target`: `integer|nil` — Follow or attack target unit.
- `separation`: `integer|nil` — Follow distance in hexes.
- `waypoints`: `Position[]|nil` — Patrol route.
- `range`: `AutopilotRangeBand|nil` — Optional engagement band.

## AutopilotOrderProgress

- `waypoint_index`: `integer` — Current waypoint cursor.
- `recovery_attempts`: `integer` — Replanning attempts for the active order.
- `stagnant_ticks`: `integer` — Ticks without route progress.
- `attack_move_origin`: `Position|nil` — Position where attack-move pursuit began.
- `attack_move_suppressed_target`: `integer|nil` — Contact already engaged during attack-move.

## AutopilotOrderRecord

- `id`: `integer` — Stable controller-local order ID.
- `order`: `AutopilotStoredOrder` — Serialized order intent; submissions use typed constants.
- `state`: `AutopilotOrderState` — Lifecycle state.
- `progress`: `AutopilotOrderProgress` — Durable execution cursor.

## AutopilotStatus

- `config`: `AutopilotConfig` — Controller settings.
- `state`: `AutopilotControllerState` — Controller lifecycle state.
- `blocking_reason`: `AutopilotReason|nil` — Reason the controller is blocked, if any.
- `revision`: `integer` — Management revision.
- `next_order_id`: `integer` — Next order ID that will be assigned.
- `active`: `AutopilotOrderRecord|nil` — Current order.
- `queue`: `AutopilotOrderRecord[]` — Queued orders.
- `feedback`: `AutopilotFeedback[]` — Recently retained outcomes.
- `next_feedback_sequence`: `integer` — Next feedback sequence that will be assigned.
- `sightings`: `table<integer, AutopilotSighting>` — Retained contact memory keyed by unit ID.

## AutopilotSubmitResult

- `ids`: `integer[]` — Assigned order IDs.
- `revision`: `integer` — New management revision.

## AutopilotContact

- `unit`: `integer` — Acquired unit identity.
- `position`: `Position` — Observed position.
- `friendly`: `boolean` — Whether the contact is allied.
- `identified`: `boolean` — Whether sensors identified the contact well enough to determine allegiance.
- `known_destroyed`: `boolean` — Whether the visible contact status reports destruction.
- `range`: `number` — Observed range in map units.
- `network_range`: `number|nil` — Shared C3/C3i aiming distance; nil without an active network.
- `relayed`: `boolean` — Seen only by network peers; the unit cannot lock or fire on it yet.
- `seen_at`: `integer` — Simulation time of the observation.

## AutopilotMemory

- `unit`: `integer` — Previously acquired unit identity.
- `position`: `Position` — Last sensor-confirmed position.
- `seen_at`: `integer` — Simulation time of the last sighting.

## Heat

- `stored`: `number` — Current stored weapon heat.
- `excess`: `number` — Sampled excess heat.

## AutopilotOwnReadiness

- `power`: `UnitPower` — Current power state.
- `maximum_speed`: `number` — Damage-adjusted maximum speed.
- `heat`: `Heat|nil` — Conventional heat state; nil for ground vehicles.
- `weapons`: `WeaponReadiness[]` — Readiness for installed weapons.

## AutopilotSighting

- `position`: `Position` — Last sensor-confirmed position.
- `seen_at`: `integer` — Simulation time of the last sighting.

## AutopilotObservation

- `unit`: `integer` — Observing unit.
- `time`: `integer` — Current simulation time.
- `position`: `Position|nil` — Own position, if placed.
- `heading`: `number|nil` — Own heading, if motion is available.
- `speed`: `number` — Own current speed.
- `own`: `AutopilotOwnReadiness` — Own mechanical and weapon readiness.
- `contacts`: `AutopilotContact[]` — Current sensor contacts, plus those relayed by active C3/C3i peers.
- `remembered`: `AutopilotMemory[]` — Fresh retained sightings.

## AutopilotFeedback

- `sequence`: `integer` — Monotonic feedback sequence.
- `simulation_time`: `integer` — Simulation time of the event.
- `order_id`: `integer|nil` — Related order ID.
- `event`: `AutopilotFeedbackEvent` — Event kind.
- `reason`: `AutopilotReason|nil` — Optional event reason.

## AutopilotFeedbackPage

- `records`: `AutopilotFeedback[]` — Retained feedback records after the cursor.
- `history_gap`: `boolean` — Whether older records fell outside the retention window.

## BtechAutopilotAPI

- `orders`: `table`
- `submission_modes`: `table`
- `fire_modes`: `table`
- `states`: `AutopilotStates` — Controller lifecycle strings.
- `order_states`: `AutopilotOrderStates` — Order lifecycle strings.
- `reasons`: `AutopilotReasons` — Blocking and outcome reason strings.
- `attach`: `fun(unit: integer, options?: AutopilotConfigPatch)`
- `detach`: `fun(unit: integer)`
- `configure`: `fun(unit: integer, patch: AutopilotConfigPatch, expected_revision?: integer):` — integer
- `submit`: `fun(unit: integer, orders: AutopilotOrder[], mode: AutopilotSubmissionMode, expected_revision?: integer):` — AutopilotSubmitResult
- `cancel`: `fun(unit: integer, order_id: integer, expected_revision?: integer):` — boolean
- `pause`: `fun(unit: integer)`
- `resume`: `fun(unit: integer)`
- `status`: `fun(unit: integer):` — AutopilotStatus
- `observe`: `fun(unit: integer):` — AutopilotObservation
- `feedback`: `fun(unit: integer, after_sequence?: integer):` — AutopilotFeedbackPage

## TacticalUnitSnapshot

- `unit`: `integer` — Assigned friendly unit ID.
- `revision`: `integer` — Management revision used for stale-intention protection.
- `status`: `AutopilotStatus` — Controller state; sightings are supplied through observation instead.
- `observation`: `AutopilotObservation` — Per-unit permitted intelligence.
- `feedback`: `AutopilotFeedbackPage` — Outcome page after the requested cursor.

## TacticalSighting

- `observer`: `integer` — Unit that acquired this sighting.
- `position`: `Position` — Last observed position.
- `seen_at`: `integer` — Committed simulation seconds.
- `current`: `boolean` — Whether this observer currently acquires the contact.
- `friendly`: `boolean|nil` — Present only for a current observation.
- `identified`: `boolean|nil` — Present only for a current observation.
- `known_destroyed`: `boolean|nil` — Present only for a current observation.
- `relayed`: `boolean|nil` — Present only for a current observation; true when only C3/C3i peers see it.

## TacticalContact

- `unit`: `integer` — Contact identity.
- `observations`: `TacticalSighting[]` — Source observations, ordered by observer ID.

## TacticalSnapshot

- `version`: `integer` — Snapshot schema version, currently 1.
- `time`: `integer` — Committed simulation seconds; restart does not advance this clock.
- `units`: `TacticalUnitSnapshot[]` — Assigned controllers, ordered by unit ID.
- `contacts`: `TacticalContact[]` — Aggregated sightings, ordered by contact ID.

## TacticalIntention

- `unit`: `integer` — Assigned unit ID.
- `expected_revision`: `integer` — Required current management revision.
- `mode`: `AutopilotSubmissionMode` — Append or replace using typed constants.
- `orders`: `AutopilotOrder[]` — Ordinary unit orders, at most 64.

## TacticalSubmitResult

- `unit`: `integer` — Controller receiving these order IDs.

## MapPointOfInterest

- `type`: `string` — Case-sensitive category chosen by the map author.
- `name`: `string` — Display name chosen by the map author.
- `x`: `integer` — Zero-based column.
- `y`: `integer` — Zero-based row.
- `elevation`: `integer|nil` — Levels above (negative: below) the hex's ground level, or nil when unset.
