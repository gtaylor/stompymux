# BattleTech porting audit (C `btmux-khi/src/btech` → Rust `src/btech`)

Audit date: 2026-09-13. Plan revised: 2026-09-14 against the current source tree.
Method: each C subsystem directory was enumerated and its behaviors, commands,
messages and persisted fields were searched for in the Rust tree. Judgements are
behavioral, not name-based. "Deferred" means outside the current scope (autopilot,
repair, naval, aerospace/DropShip, infantry, battle armor, unit loading); those
items are inventoried but not prioritized.

The per-section bodies below carry dated implementation and verification notes
from the sessions that worked the original plan. Several original claims were
corrected by reference tracing (LZ glyphs, minefield removal on clear, `@stat`
naming, short-row tolerance, `Exceptional_Attribute`/Lives, radio negative
frequency, VTOL taxi/bridge/crash paths, cables as MechWarrior actions). The
summary table and the plan at the end reflect the 2026-09-14 state; the ledger
of individual changes is in [porting-audit-progress.md](porting-audit-progress.md).

## Summary by subsystem

State as of 2026-09-14. "Headline gaps" lists only what is still open.

| C subsystem | Lines | State | Headline gaps |
| --- | ---: | --- | --- |
| commands | 1.8k | mostly ported | `perecm`/`pereccm`, `@createbays`; `embark`/`disembark`/`udisembark` and `attachcables`/`detachcables` need the MechWarrior unit; no shared `common_checks` admission (3 divergent messages, no `last_use` reset). Per-class restricted gate and in-cockpit `HELP` are done. |
| core | 2.3k | partial | RNG is ChaCha8 not xoshiro256**; no event queue / per-kind `EVENTSTATS`; 14/22 diagnostic channels missing |
| scripting | 6.9k | redesigned | ~100/112 C Lua API names missing; 0/67 `bt*()` softcode functions; 0/12 Lua constant namespaces; deferred-family `@setspecial` values inert |
| unit | 20.5k | mostly ported | Mech ICE/Waterproof/TargComp flags inert; OmniMech, ForceSingleHS, NoSensors; `standanyway` inert; ejected-MechWarrior class absent |
| combat | 24k | ported (in scope) | Autoeject (needs MW class); deferred-family hit tables. To-hit terms, range/woods damage, rotor divisor, underwater firing and sixth sense are done. |
| movement | 11k | ported (in scope) | Water-depth MP penalty is a behavior decision (unreachable in live reference); cables are MW actions. Periodic piloting, sprint, stagger-at-action, VTOL collisions are done. |
| sensors | 8.5k | mostly ported | PERECM/PERECCM; `MOVE_NONE` stationary Mechs; post-detection perception XP; DropShip detection factor (deferred). `view`, datalink, radar, light re-check, SENSOR/SCAN/REPORT/C3 formats are done. |
| map + special | 7k | mostly ported | `@btech/register|unregister` and per-object handler dispatch; destroyed-building rebuild is a behavior decision (unreachable in reference); DS map channel messages (deferred). Load tolerance, eternal fire, `LOADMAP` replies, TBITS, `LIST`/`FIXMAP`/`SETCOND` output, step-on-base notice, HELP and restricted admission are done. |
| ui | 9.6k | mostly ported | Interactive coolmenu; deferred-family status blocks, armor templates, LRS glyphs and DropShip footprint; `PersonalECM`/`Carrier:` technology lines; per-player Lua status template; repair `?` fill. `weaponspecs`, `+rolls`, banners, base-entry wording are done. |
| character | 4.2k | mostly ported | Chargen (no executable reference; needs a contract); personal-combat armor table and loadout columns; `initialize_pc` MW construction; `Extra_Edge` review. Advantages, computer XP, eject XP, counters, `+charclear`, `+show`, `character.list` are done. |
| economy | 1.6k | partial/deferred | part costs, unit cost (`mech_fasa_cost`), brand collapse |
| persistence | 6.5k | schema ported | Schema identical (43 tables, 0 drift). 13 `btech_mech*` tables + autopilot/repair/costs/unit_configuration never read or written |
| repair | 9.2k | deferred | nothing ported except building repair and turret unjam |
| autopilot | 11k | deferred | nothing ported; schema and command metadata present |

## Commands and admission

**Missing in-scope commands:** `embark`, `disembark`, `udisembark`, `attachcables`,
`detachcables`, `perecm`, `pereccm`, `@createbays`. Cockpit `view <target>` is now implemented; see Sensors.

**Missing deferred commands:** aerospace (`climb`, `dive`, `thrust`, `enterbay`,
`checklz`, `bomb`), battle armor (`thrash`, `attackleg`, `swarm`, `jettison`),
repair (21 tech-console commands plus `@magic`, `@fixextra`).

**Admission layer.** C runs `common_checks` on every command with these messages:
`You are destroyed!`, `Reactor is not online!`, `You are momentarily blinded!`,
`You are unconscious....zzzzzzz`, `Now now, only the pilot can push that button.`,
`You are on no map!`, `You are on an invalid map! Map index reset!` (with auto
shutdown). Rust has partial checks in `power.rs` reached per operation with different
text (`Start the unit first`, `You are unconscious`, `Take the cockpit with pilot
first`). Also missing: `mech_last_use_reset` on every command, and the per-unit-class
command gate (`btech_command_allowed_for_mech`, `Sorry, that command is restricted!`).

**Help.** C has a per-command help string and a grouped in-cockpit `HELP` listing.
**Implemented (2026-09-14):** per-type command metadata lives in
`special_commands.json` and `special_help.rs` renders `HELP`/`HELP ALL` from it;
`CommandDefinition` itself still carries no help text. General MUX help remains
the markdown topics.

**Debug catalog.** All 9 present; `shutdown <map#>` is renamed `clearmechs`;
`eventstats` reports object counts, not per-event-type counts.

## Core runtime

- **RNG**: C xoshiro256** with splitmix64 seeding; Rust ChaCha8. Identical seeds
  will not reproduce reference outcomes. Roll-statistics histograms and the `@stat`
  readout are gone.
- **Events**: C has a typed scheduler with 79 named `EVENT_*` kinds. Rust encodes
  deadlines as state fields advanced by the tick. Fine idiomatically, but
  `EVENTSTATS`, `btlag`, and durable repair/autopilot timers depend on it.
- **Channels**: `MapErrors` and `MechDebugInfo` were added. Still missing:
  ScenErrors, ScenStatus, MechAI, MechCustom, DBInfo, MechDeaths, MechErrors,
  EventInfo, MechSensor, MineTriggers, DSInfo, MechAttackEmits, MechAttacks,
  MechTechXP, TACInfo.
- **Config keys**: all 80 `btech_*` runtime keys exist. The previously unconsumed
  `moddamagewithwoods`, `moddamagewithrange` and `divrotordamage` now feed shared
  aiming/damage and vehicle-impact rules. Remaining behavior gaps are tracked
  under Combat; the presence of consumers alone does not establish full parity.
- **Per-object configuration** missing from the Lua surface:
  `unit.assigned_pilot`, `unit.preferred_id` getter, `player.mechwarrior_template`,
  `player.loadout`, `repair.technician_available_in`.

## Scripting (Lua and softcode)

The Rust Lua package is a command mirror (269 functions), the C package a data/admin
API (112). Only 9 names match. Missing with no generic fallback:

- `repair.*` (5), `autopilot.*` (7), `system.event_lag`, `system.units_in_zone`.
- `parts.categories/list/search/resolve/set_cost`.
- `template.*`: `exists`, `engine`, `battle_value`, `base_cost`, `payload`, `armor`,
  `installed_parts`, `critical_slots`, `weapons`, `technologies`, `show_status`,
  `show_weapon_specs`, `show_critical_status`.
- `unit.*`: `apply_damage`, `piloting_check`, `load_template`, `effective_max_speed`,
  `section_condition`, `radio_channels`, `battle_value`, `engine`, `payload`,
  `installed_parts`, `technologies`, `critical_slots`, `armor`.
- `map.*`: `blast_zones`, `in_blast_zone`, `line_of_sight`, `units`, `unit_by_id`,
  `range`, `place_unit`.
- `character.catalog`.
- All 12 constant namespaces (`unit.types`, `unit.sections`, `unit.fire_modes`,
  `repair.operations`, `autopilot.orders`, terrain/LOS contracts, ...).
- All 67 `bt*()` MUX softcode functions. Existing world attributes using them break.
- `@setspecial`/`@viewspecial` values: `buildflag`; `bay0-3`, `carmaxton`, `MaxSuits`,
  `si`, `si_orig`, `SwarmedBy`, `SwarmTarget`, `computer`, `radio`, `perception`,
  `stall`, `techtime`, and the career counters `shots_fired/hit/missed`,
  `damage_inflicted/taken`, `hexes_walked`, `units_killed`.
  The three shot-counter fields are now implemented through shared unit storage
  and firing transactions; see the Character and experience clarification below.

## Unit construction and equipment

Weapon (152/178, delta is personal-combat and infantry), ammo-mode (23/23), gyro,
cockpit, MASC/TSM, ECM/C3/TAG/NARC/Artemis catalogs are ported. Gaps:

- **CASE II** is implemented as `BattleSystem::CaseIi` (`CASE-II` criticals), with
  the venting rule described in `btech-coverage.md`.
- Mech `ICEEngine_Tech`, `Waterproof_Tech`, `TargComp_Tech` (chassis flag),
  `OmniMech_Tech`, `ForceSingleHS`, `NoSensors`, `SS_Ability` and `CompactHS` have no
  Mech rules yet. Hardened armor, reinforced and composite structure, small cockpit,
  laser heat sinks, Watchdog and Artemis V are implemented (see `btech-coverage.md`).
- `OMNI_BASE_MODE` critical bit is not distinguished.
- `MECHPREF_STANDANYWAY` is persisted but inert (stand still blocked on high BTH).
- Ejected MechWarrior unit class (`CLASS_MW`) does not exist; ejection and hit/LOS
  special cases for it are absent. `max_suits != 0` rejects the template.
- Softcode part-name surface (`btpartname`, `btpartslist`, brand name forms).
- Not persisted: `si/si_orig`, `carmaxton`, `maxsuits`, `bay[4]`, `sspin`,
  `autopilot_num`, `infantry_specials` (all deferred domains).

## Combat

Firing pipeline, weapon modes, special rounds, cluster tables, Mech/vehicle/VTOL hit
tables, damage transfer, criticals, CASE, physicals, artillery, mines, AMS, TICs,
C3 targeting, aimed shots and self-destruct are ported. Gaps:

- **To-hit**: `+1` attacker in water; `+1` vs moving VTOL; LBX `-3` vs VTOL (Rust
  flat `-1`); stinger `-3` vs airborne / `-1` vs out-of-control; woods cover `-1/-2`
  (`moddamagewithwoods`); underwater weapon range profiles.
  **Underwater combat (2026-09-14):** the shared Rust catalogue now
  exposes all 32 active water profiles, optional long bands, extended limits and
  rounded water accuracy through `BattleWaterRanges` and
  `BattleWeapon::water_range_modifier`. Boundary tests include PPC zero/minimum
  penalties and small lasers without a long band. Unit and coordinate aim now
  select water bands from mounting-section submersion before C3 and stealth.
  Water C3 uses normal physical water reach and peer water brackets; live network
  and restart checks cover the limit. The enclosing raw PPC minimum still takes
  precedence over water bracket arithmetic. Live sighting, unit and coordinate
  firing now admit water-capable mounts through one shared gate. Submersion is
  carried through the existing private damage contexts; range-based energy
  damage uses the water bands before glancing, including absent-long-band
  halving. Native/Lua shots, both target anatomies, rollback, restart and blocked
  waterline visibility are covered by `tests/btech_underwater_fire.rs`.
  **Verification note (2026-09-14):** the configured occupied-woods accuracy
  credit now uses shared target terrain/elevation for every supported shooter
  and target chassis. Its -1/-2 subtotal term, canopy boundary, overlay handling
  and configured native/Lua sighting are tested. Damage absorption, woodland
  consequences and their feedback remain incomplete; the overall option is
  not yet fully ported.
  Single-hit armor damage now subtracts 2/4 before glancing, retains a minimum
  of one and invokes the existing woodland resolver with pre-absorption damage.
  Native/Lua firing, both target families, feedback, terrain clearing and
  rollback are tested. Missile armor damage now subtracts from the total after
  interception and rounds down to whole missiles; woods can absorb all survivors.
  Inferno bypasses armor absorption, and swarm continuation counts missiles
  absorbed by woods as spent. Burst shells now share per-shell absorption, a one-damage floor before glancing,
  and one terrain effect/notification per attack across Mechs and vehicles.
  Glancing cluster-count adjustment is retained. Successful thermal-only hits
  now run the shared woods terrain check and feedback without reducing flamer
  heat or coolant strength. LBX hits now apply the nominal-damage terrain check
  before pellet absorption, using the forest density left by that check and
  preserving one damage per surviving pellet. Launched non-missile misses now
  perform the reference incidental terrain check independently of the woods
  damage setting, without absorption feedback or target damage. Burst glancing now halves surviving shell damage independently
  of the woods setting, while preserving its separate hit-count adjustment.
- **Damage**: range-modified energy damage (`moddamagewithrange`: `+1` at 1 hex,
  `-1` past medium, halved past long); woods absorption `-2/-4` with
  `The woods absorb some of your shot!`; VTOL rotor divisor (`divrotordamage`).
- **Terrain**: a cleared woods hex should also remove its minefield.
  **Artillery/cache implementation (2026-09-14):** deposition now uses the
  reference cache-dependent duplicate gate and does not rebuild coverage.
  Tests cover repeated uncached deposition, saved inactive fields, later rebuild
  and suppression after coverage exists. Broader consumer acceptance remains in
  `docs/btech-map-bits.md`.
  **Verification correction (2026-09-13):** the reference call reaches
  `combat/mine.c::mine_field_possibly_remove`, whose implementation explicitly
  disables removal until trigger mines can be distinguished. Rust preserving
  minefields matches the executable reference behavior. Actual removal is a
  potential gameplay change, not a porting omission.
- **Autoeject** on section/head destruction and pre-reactor-explosion; spawns a MW
  unit. Rust evacuates crew to a room instead. Needs `CLASS_MW`.
- **Sixth sense** lock warning table (9 messages by range band and tonnage).
  **Implemented (2026-09-14):** startup captures the current player's Sixth_Sense
  advantage. Explicit unit locks share a 2d6 <= 8 check and a 1–3 second delay,
  with all nine reference messages selected by range and current mass difference.
  Observers do not trigger warnings. Each independent event persists on its
  recipient unit; delivery checks the current pilot's connectivity and
  consciousness and sends private output. Native, Lua and gunner locks use the
  same service. Coordinate locks and clearing selection do not schedule events.
- Deferred: aero/DS/naval/BSuit/infantry hit tables, `thrash`, MW damage multipliers.

## Movement

Falls, hull-down, MASC/supercharger, motive rules, orbital drop, map linking,
stacking, heat interaction and movement XP are ported. Gaps:

- **Per-turn piloting checks**: running with damaged
  gyro/hip (`Your damaged mech falls as you try to run!`); low-gravity overspeed leg
  damage (`Your legs take some damage!`); shut-down units re-roll at +3 every turn.
  **Characterization and prerequisite fix (2026-09-14):** the damaged-gyro/hip
  branch actually runs every heartbeat, outside the turn-boundary and airborne
  guards. The heartbeat calls piloting only for started units or unconscious
  pilots. Shutdown therefore reaches this path only for an unconscious pilot,
  and the shared check fails without rolling. Odd ticks round up before the
  turn test, so ticks 29 and 30 both qualify. The gravity branch requires special
  conditions and nonstandard gravity, with actual speed above the unloaded
  maximum; it is not explicitly restricted to gravity below 100. The periodic
  hook is now implemented with a persisted global phase and shared fall/internal
  damage services. Failed world saves roll back the clock, dice, damage and
  staged output together. Idle ticks advance the clock, which resumes from its
  saved phase without offline catch-up. Acceptance covers gyro/hip running
  thresholds, hot TSM turn behavior, gravity gates and ordered biped/quad leg
  damage, shutdown/unconscious admission for all supported chassis, saved replay,
  and active/idle server commit failure. Periodic checks now publish the reference
  two-line piloting-roll feedback before their damage/fall consequences. The
  assigned recipient is captured before those consequences; without a pilot,
  feedback uses the reference cockpit audience. Automatic and blocked checks
  stay silent. Unjamming and orbital landing share the same formatter.
  Standing, controlled prone drops, bootlegger pivots and manual early jump landing now capture
  ordered private roll feedback as well. Their host adapters share the movement
  notice publisher, preserving introductory messages before the check and
  consequences after it without exposing the roll to cockpit passengers or
  outside observers. Automatic standing and slow prone drops remain silent.
  Vehicle water avoidance, tree/rock avoidance and bridge-underside checks now
  retain the same private feedback after their terrain warning. Movement phases
  merge reports through the shared offset-aware publisher data, retaining pilot
  notices when vehicle and aircraft outcomes join Mech movement. Pilotless terrain
  exemptions still consume no roll and produce no roll feedback.
  Cliff and reverse-slope checks now retain feedback for both ground chassis
  families, including auto-fall's silent exemption. Mech water-entry checks also
  publish their actual roll. Ground segments and interrupted-jump settlement
  preserve these private notices through the shared insertion-offset helper.
  VTOL forest and elevation collisions now preserve the same warning/roll/outcome
  order, including the pilotless elevation and inactive-pilot forest exemptions.
  Their detached environment reports capture the recipient before crash effects;
  the flight update carries that feedback into the shared movement publisher.
  Jump elevation avoidance and completion checks for stagger, leg damage and
  gyro damage also capture private feedback. Nested landing reports preserve
  insertion positions for ordinary, obstacle and early landings. Launch-time
  stagger checks use the same ordered publisher for conventional jumps and DFA,
  including failed attempts and subsequent private destination rejections.
  Kick, trip and missed-mace balance checks now capture the balancing unit's
  pilot feedback. Single physical attacks and arm sequences preserve its ordered
  position through the shared notice publisher. Critical-damage balance now
  captures the pilot and ordered feedback in tactical impacts and salvo groups.
  Direct ammunition-explosion and physical-attack publication preserve it;
  ordinary Mech and vehicle shot assembly now offsets those notices through
  salvo and firing-message prefixes. Configured native/Lua fire and direct shot
  actions publish them privately. Swarm flights also retain private feedback
  through secondary targets for both launcher families. Launch-misload feedback
  now passes through the shared failure formatter for unit, hex and artillery
  attacks. Other nested damage aggregators still require integration before critical-damage feedback can be
  considered complete.
  Heavy Gauss recoil now captures the shooter pilot before any fall and shares
  one warning/roll/fall formatter between unit and hex shots. Stationary shots
  retain their no-check behavior.
  Periodic stagger checks now capture private roll feedback after the stagger
  warning and before fall messages in all three history modes. Blocked checks
  retain their silent no-roll behavior, and the host uses the shared publisher.
  Thermal shutdown balance captures its pilot before power-down clears the
  assignment. Thermal report formatting adjusts private positions for inserted
  Computer diagnostics and retains heat-triggered ammunition impact feedback.
  Stacking host actions now retain avoidance-roll feedback and nested collision
  critical-balance feedback. Movement, jump settlement, orbital landing, thermal
  falls and shutdown preserve its insertion position, including bulk map clearing.
  Critical-impact-triggered airborne falls now preserve nested stacking feedback
  as well. Host damage retains collision impacts and falls on the balance report,
  whose shared publisher handles their XP and character consequences. Host attack
  capability survives an ordinary primary target, permitting an in-character
  secondary collision target; pure tactical entry points keep their rejection
  and rollback contract for unsupported character consequences.
  Charge and DFA reports now retain nested impact feedback and capture their
  own balance rolls, including DFA miss injury checks. Mutual charges offset
  both attempts into one ordered stream. Ground and jump movement preserve
  that stream when they trigger collisions or complete DFA landings; direct
  host actions use the same private publisher.
  Fall reports now capture the pre-injury pilot and format the protection roll
  before injury and damage messages, retaining private feedback from damage
  groups as well. Direct falls, critical balance, physical attacks, charge/DFA,
  recoil, shutdown, stacking, stagger, standing/prone, bootleggers, orbital
  landing, jump settlement, cliff/water movement and thermal falls preserve
  that stream. Periodic checks also retain nested gravity-impact feedback.
  Free-fall landings preserve private protection feedback after the impact
  warning, and MASC failure falls publish their warnings and checks before
  character/XP consequences. Surface-break and section-exposure reports now
  retain private fall feedback. Direct surface/flooding actions, damage impacts,
  Mech falls, prone/DFA immersion and ground/jump/orbital movement preserve it,
  including vehicle ground movement that breaks occupied ice. Pickup preserves
  flooding and ice-break private feedback and uses the full section-exposure
  consequence publisher. Weapon surface impacts retain fracture feedback through
  the shared hex-shot formatter. Vehicle falls now retain their protection rolls
  and neighboring ice-break feedback through shutdown, driving, obstacles, VTOL
  crashes, periodic checks and orbital movement. Upward ice breakout and VTOL
  crash prefixes offset the private stream before publication. Shared blast
  packets preserve private impact feedback; artillery arrivals and wizard packet
  damage publish it in order. Mine blast, activation and command reports now
  retain that stream through direct detonation, falls, prone transitions, ground
  movement, jump settlement and VTOL touchdown. The direct vehicle-fall action
  also uses the ordered publisher. Reactor reports preserve packet and secondary
  blast feedback through section exposure and tactical impacts; instability
  warnings offset the private stream before direct/Lua/countdown publication.
  VTOL engine-loss checks now retain private feedback through vehicle critical,
  internal, armor and impact reports. Direct critical/armor actions and shared
  blast packets publish it. Shared vehicle-salvo formatting preserves packet
  checks for both shooter families; direct and coordinate launch misloads retain
  the private stream too. Vehicle inferno, heat, terrain exposure and scheduled
  fire reports now retain nested private checks; direct actions, ground movement,
  blast heat and vehicle-salvo formatting forward them. Failed iNarc swats also
  retain self-damage and fall feedback through the pod-removal host action.
  Heartbeat checks now send the reference subtotal diagnostic to MechDebugInfo
  before cockpit feedback, with silent automatic/blocked checks and atomic channel
  rollback. The shared formatter retains the distinct awarding/non-awarding labels.
  Conventional unjamming also emits its diagnostic immediately before cockpit
  feedback through the shared host/server publisher; rotary and skipped checks
  omit the piloting diagnostic.
  Standing, controlled fast drops and bootlegger turns now publish their initial
  check diagnostics through one ordered maneuver publisher. Standing uses the
  non-awarding label; drops and bootleggers use the reference `(noxp)` label for
  awarding checks. Pre-roll maneuver notices remain before the diagnostic, which
  precedes private roll feedback. Automatic checks stay silent, and channel
  publication failures restore the entire maneuver. Traditional, retained-history
  and consumed-history stagger checks now capture an explicit diagnostic boundary
  after severity warnings, including cockpit fallback when no pilot is assigned.
  The shared publisher preserves channel/roll ordering and channel failure rollback;
  blinded checks remain silent. Other diagnostic callers, including nested fall
  checks, remain follow-up work. Computer failures and
  durable recovery are now implemented as detailed in the Character section.
  Its shared control service now correctly blocks
  unit-owned recovery and blindness, in addition to assigned-player recovery
  and shutdown, without consuming dice. Already-prone Mechs retain automatic
  success before those gates.
- **Water speed**: Mech loses 1 MP at depth -1 and 3 MP at depth <= -2; Rust only
  caps desired speed on entry.
  **Verification correction (2026-09-14):** the dormant terrain-speed branch
  would divide speed by two or four, rather than subtract fixed MP. More
  importantly, live `mech_speed_update` supplies `mech_position_elevation`, which
  reads the nonnegative encoded depth through `mech_hex_elevation_get` and
  `map_elevation_get`. The codec rejects negative elevations. Its `== -1` and
  `<= -2` checks therefore never run for ordinary loaded maps. This differs from
  `battle_map_hex_elevation`, which explicitly negates water/ice depth. Whether
  to enable intended depth slowing or retain live reference behavior is an open
  user choice; no movement penalty has been added pending that answer.
- **Sprint mode** state and its speed ceiling, TSM bonus, speed-demon MP, and
  `You can not backup while sprinting!`; also `You can not backup while towing!`.
  **Inventory clarification (2026-09-14):** the towing reverse guard already
  exists in shared `motion_controls::require_reverse_allowed`, called by Mech
  and vehicle controls. `tests/btech_towing.rs` covers native/Lua/direct requests,
  the SalvageTech exception, detached units and restart across mobile chassis.
  Sprint remains open. Rust now stores the mode in both unit families, accepts
  secondary-status bit 16, rejects reverse/charge selection and clears the mode
  after admitted orbital insertion. Shared speed bonuses now reach configured
  throttle, status, ground motion, turning and VTOL flight. Saved-replay tests
  cover acceleration, clearing the mode at speed and new commands using the
  reduced ceiling across six mobile chassis. Configured TSM sprint policy also
  reaches XP/BV, jump cargo admission and predictive firing; tests verify opposite
  jump admission and distinct predicted hexes under the two settings. Cargo plus
  gravity now has effective/throttle/acceleration/replay coverage across six mobile
  chassis; a vehicle effective-speed gravity omission was fixed. The same matrix
  now covers ordinary mode, whose throttle, turning and movement also apply map
  gravity. A further 48-case matrix covers combined MASC/supercharger/sprint,
  cargo and gravity through actual acceleration and replay; hot-TSM policy changes
  while moving are also covered. First tail-rotor damage now limits the loaded
  sprint/gravity velocity budget, and repeat damage preserves controls. Raw mode
  import remains open in `docs/btech-sprint.md`. No ordinary sprint-enable command or
  MOVEMODE event scheduler was found in the reference call sites. The live speed
  calculation adds sprint alongside MASC/supercharger, gates the hot-TSM increment
  with `btech_tsm_sprint_bonus`, and grants one MP for boolean Speed_Demon when no
  movement-mode event is pending. Charge rejects sprinting and orbital-drop
  admission clears it. The `btech_sprint_bth` accessor has no production callers;
  its mere configuration presence is not evidence of an active to-hit rule.
  The live calculation order, admission precedence, Rust caller inventory and
  required acceptance cases are recorded in `docs/btech-sprint.md`.
- **Stagger at action time**: pre-jump PSR, jump-landing PSR, prone drop-level and
  BTH additions.
  **Reachability characterization (2026-09-14):** these checks read the separate
  `rd.stagger_damage` scalar, not the rolling damage-history list. Live source
  writers reset the scalar to zero or set it to -10; ordinary incoming damage
  appends history without increasing it. `StaggerDamage` is read-only in the
  script catalog and setter. SQLite runtime restoration can nevertheless restore
  a positive scalar, so this is not globally unreachable. Exact restored-scalar
  representation is now present in Rust-owned `BattleStagger.action_damage`.
  Controlled drops apply its positive twenty-point levels to the roll and force
  a minimum level-one fall when necessary; status and read-only `StaggerDamage`
  inspection use the same value. Successful/failing drops retain this scalar
  while clearing rolling history as configured. Landing now checks the scalar
  after DFA/surface handling and before damaged-leg/gyro rolls. Its modifier
  always includes weight class. Failure uses the shared level-one fall, retains
  stabilization and stops normal landing continuation. Projected/DFA
  launch attempts now use the same modifier and commit an immediate level-one
  fall on failure, without creating a flight. Native and Lua share injury/XP/
  casualty publication and rollback. Native argument parsing, default-target
  resolution and route validation follow the stagger roll. A failed roll stops
  before those checks; a successful roll remains committed if they later reject
  the request, with a private pilot message. Ordinary admission (including the
  underground ceiling) still rejects without rolling. Completed landing now
  resets the scalar in traditional mode only; the early stagger-fall return
  retains it. Retain/consume modes preserve it. Tests verify that reference-style
  map reassignment preserves scalar/history, while core destruction resets the
  scalar. Broader command syntax/message parity, restored scalar-event handling
  and C snapshot import remain open. Recent-hit sums must not activate the scalar.
- **Landing crew gate corrected (2026-09-14):** cockpit assignment no longer
  stands in for consciousness. Landing now shares the unit-owned/assigned-crew
  recovery query with piloting. Conscious unassigned crews land normally;
  unconscious crews fall with the reference warning and without the normal
  jump-completion message. Biped/quad tests cover both assignment states,
  real tactical injury, all flight ticks and persisted deterministic replay.
- **Jump admission**: cargo weight refusal (`No, with this cargo you won't!`), iNARC
  pod-removal busy check.
  **Review (2026-09-14):** the cargo gate is already implemented in the shared
  jump launch path. The reference schedules `EVENT_REMOVE_PODS` through the
  ground-vehicle/VTOL `REMOVEPODS` command; Mechs use immediate individual pod
  swats. Ordinary Mech play therefore does not create that busy event. Restored
  reference events remain a separate import concern. During this review, vehicle
  pod removal was found to lack the VTOL landing gate. It now rejects airborne
  and falling aircraft with the reference reply, after the motion check and
  before crew busy checks. Landed and launch-preparation states remain eligible.
  Native/Lua, refusal precedence, unchanged rejected state, rollback and restart
  are covered by `tests/btech_vtol_pods.rs`.
- **VTOL**: landed flyer taxiing onto forbidden terrain (PSR +5 or crash); bridge as
  a landing surface; backwalk fall levels for landed flyers; crash fall level formula
  differs (`drop_height + 1` vs `1 + |vspeed|/10.75`).
  **Landing review (2026-09-14):** deliberate bridge landing is not a porting
  gap: reference `aero_land` permits grassland, road and (for VTOLs) building,
  exactly the Rust surface gate. The separate taxi transition permits bridges;
  those two rules must not be conflated. Rust's controls and movement dispatcher
  currently exclude landed VTOL movement; the reachability review below
  distinguishes control admission from actual position integration.
  Landing now uses the reference fuel, already-landed, altitude, horizontal-speed,
  vertical-speed and terrain refusal wording, plus the VTOL touchdown message.
  Launch cancellation names the acting pilot with literal-name escaping through
  native and Lua output. Tests cover exact replies, unchanged refused state,
  cancellation rollback and restart. Other landing refusal/admission differences
  remain separate from the collision and taxi reachability reviews below.
  **Crash-path review (2026-09-14):** the formulas above belong to distinct
  reference paths. `mech_vtol_altitude_check` uses
  `1 + trunc(abs(vertical_speed) / MP1)` for vertical surface contact, matching
  Rust's `BattleVtolMotionStep::surface_contact`; replacing that formula with
  drop height would introduce a regression. `flight_hex_transition_resolve`
  uses `drop_height + 1` after rolling back a horizontal terrain collision.
  That airborne horizontal collision path is implemented in the elevation-entry
  review below; vertical surface contact retains its separate speed-based formula.
  Rotor-loss landing refusal now reads `The rotor's dead!`, with fuel refusal
  retaining precedence and rejected material state unchanged.
  **Taxi reachability review (2026-09-14):** `mech_motion_integrate` returns
  false immediately for landed `MOVE_VTOL`; `mech_movement_update` then returns
  before hex transitions. Landed taxi integration instead belongs to `MOVE_FLY`,
  whose movement branch sets `update_surface` and reaches the shared flyer
  transition handler. Thus the landed forbidden-terrain and reverse-slope taxi
  branches are not ordinary VTOL movement requirements. Aerospace taxiing stays
  with the deferred aerospace family. Reference `mech_speed` does admit landed
  VTOL throttle requests. Rust now admits horizontal throttle while landed or
  preparing to launch through the shared vehicle controls. The aircraft stays
  in place, and liftoff clears actual and desired horizontal speed before
  climbing at 60 KPH. Native/Lua agreement, callback rollback, persistence and
  countdown behavior are verified. Grounded heading selection is now admitted,
  and horizontal throttle checks fuel through the shared vehicle control path.
  Native and Lua both honor the configured fusion exemption; ICE engines still
  require fuel. Heading selection and readouts remain available without fuel.
  Tests cover landed, launch-preparation and airborne phases, exact empty-tank
  refusal, unchanged rejected state, callback rollback and restart. This covers
  command admission and grounded event timing: reference `mech_move_event`
  returns for landed VTOLs before both heading and speed updates, so the chosen
  bearing is retained without turning during grounded/countdown ticks. Tests
  verify that bearing survives liftoff while throttle resets.
  **Forest entry (2026-09-14):** crossing into light or heavy forest below
  surface elevation plus two now requires an active pilot and a +5 piloting
  check. Success restores the pre-entry position at the previous terrain's
  elevation and stops horizontal
  motion; failure or disconnected crew applies a one-level crash at the entry.
  Shared piloting, XP, crash, hiding and host publication services own the effects.
  The material path returns an unresolved forest contact without mutation until
  the host supplies pilot state. Tests cover canopy boundaries, hovering within
  a forest hex, the +5 threshold, connected/disconnected crew, exact messages,
  one-level damage, restart replay and live movement publication.
  **Elevation entry (2026-09-14):** horizontal obstacles share the jump-entry
  terrain predicate, with the flight collision check's intact-ice surface rule.
  The host rolls back to the previous hex and terrain elevation before rolling
  piloting with `trunc(rollback_elevation / 3)`. An unassigned pilot succeeds
  automatically; assigned pilots also require grass, road or building beneath
  the rollback position. Success lands, stops actual/vertical speed and retains
  desired horizontal speed. Failure uses the shared aircraft crash resolver.
  The reference drop-height helper subtracts the selected surface twice, so the
  resulting severity can be zero or negative on raised terrain. Shared fall
  material now retains that signed crew modifier while structural damage floors
  at zero. Tests cover positive/zero/negative severity, terrain admission,
  pilotless success, slow entry deferral and save/reload. Domino is a no-op for
  VTOLs in the reference because its resolver requires `CLASS_MECH`.
  This review also corrected forest avoidance rollback: reference
  `mech_hex_entry_resolve` supplies the previous map elevation, not the prior airborne
  altitude, to `mech_position_rollback`.
- `attachcables`/`detachcables` (third-party cable attach, in scope).
  **Prerequisite verified (2026-09-14):** both commands have class mask 128,
  `GFLAG_MW`, in the reference command registry. They are on-foot MechWarrior
  actions, not additional Mech/vehicle cockpit commands. Completing them requires
  the still-missing MechWarrior unit model and its actor admission; the existing
  shared towing relationship and target preparation can then serve the action.
- Skid message per motive type (`goes into a skid!` vs `skids to a halt!`).
- Movement message text is not preserved (e.g. `Desired speed changed to %d KPH.`).

## Sensors

Sensor table, IR/EM/seismic/radar formulas, probes, LOS, contacts, ECM/ECCM/Angel,
stealth, C3/C3i, TAG, NARC, searchlight and sensor damage are ported. Gaps:

- **`view <target>`** unit markings command was missing.
  **Implemented (2026-09-14):** cockpit and gunner viewing now share running
  admission, acquired-contact visibility and unblocked LOS without a scan-range
  limit. Markings are bounded literal text, stored with both unit families;
  `unit.markings`, `unit.set_markings` and `unit.view` expose configuration and
  viewing to Lua. Native VIEW dispatches by location, retaining the wizard gate
  for map-room VIEW. Tests cover all supported chassis, restart, hidden contacts,
  independent gunner targeting, configuration authority and rollback.
- **Personal ECM** (`perecm`/`pereccm`) hardcoded Off.
- **Artillery forward-observer datalink** in `spot` (delayed link, 2x radio range,
  movement cancels, `Data link established with %s.`, periodic link-break).
  **Contract characterized (2026-09-14):** this path requires an eligible
  numbered artillery weapon and no LOS; weapon or section recycling prevents
  eligibility, while empty ammunition alone does not. Range uses the spotter's
  radio, delay is `2 * (trunc(range) / 10 + 5)` seconds, and maintenance runs every
  ten seconds. Movement cancellation requires all four captured X/Y comparisons
  to differ, not merely movement by one unit. Multiple pending requests can
  coexist and survive clearing a selection. The detailed contract and remaining
  acceptance requirements are in `docs/btech-spotter-datalink.md`.
  **Shared lifecycle implemented (2026-09-14):** Mechs and vehicles now persist
  independent ordered requests and ten-second maintenance events. Native and
  Lua publish both participants' notices transactionally; shutdown retains
  requests and destruction cancels them. Tests cover every supported chassis,
  radio and delay boundaries, all coordinate-change combinations, repeated
  requests, restart and server save failure. Correction resets now distinguish
  direct selection, self-declaration, clear and observer withdrawal. Actual
  biped/quad artillery launches after completion are tested. The contract lists
  remaining edge-case acceptance; this does not close other sensor gaps.
- **Radar**: the flying-type `-3` aim modifier is implemented, including low
  VTOL elevations. Eligibility and acquisition remain separate checks; boundary
  and live restart coverage is in `tests/btech_radar.rs`.
- **Stationary 140% reach** not applied to active probes; keyed on vehicle
  Stationary only, not `MOVE_NONE` Mechs.
  **Verification correction (2026-09-13):** `active_probe::evaluate_contact`
  already calls `sensors::sensor_maximum`. New world-level boundary tests verify
  fixed-vehicle probe ranges of 8/4/11 versus ordinary 6/3/8. The remaining
  `MOVE_NONE` Mech construction/representation gap is separate.
- **Detection**: dropship x4 chance; post-detection perception XP (1 in 6).
- **LOS**: dug-in eye height `z + 0.1`; 180-hex cutoff with `AA_TECH`.
- **Light re-check** after map light change (`It's now too dark to use %s!`).
  **Implemented and corrected (2026-09-14):** supported sensors have no positive
  minimum-light requirement, so valid map light cannot trigger the dark warning.
  Light-amplification has maximum light 1; a change to day emits
  `The light's kinda too bright now to use Light-amplification!` to running,
  surviving units' cockpits. The recheck preserves locks and pending requests.
  It checks distinct active modes: an L/L pair becomes V/L, matching the
  reference's identical-secondary skip. Unchanged light does not recheck.
  Native map conditions and native/Lua map-field actions publish consequences
  transactionally. All supported chassis, slot pairs, shutdown, restart and
  callback rollback are tested.
- **Output format**: `SENSOR` header/Primary/Secondary/Wanted layout, `(R:...)`
  suffix, arc annotation, verbose via any argument; `SCAN`/`REPORT` 6-space block
  (arc lines, flags, jumping/vertical/lateral/turret lines, `WEAPON SYSTEMS` header);
  `C3TARGETS` `c:` column and `something` degradation; TAG messages (`Out of range!
  TAG ranges are 5/10/15`, destroyed vs not-equipped distinction).
  **SENSOR implemented (2026-09-14):** one renderer now provides the reference
  matching-mode line, mixed-mode headers and slot rows, all nine sensor
  descriptions, optical arc annotations and compact range suffixes. Any one
  argument requests verbose active descriptions; pending selections always use
  compact Wanted output. Native inspection and `unit.sensor_report` share the
  renderer without advancing simulation state. Other listed report formats
  remain separate outstanding work.
  **C3 target format verified (2026-09-14):** classic C3 and C3i share the
  existing network target renderer, including the fixed-width `c:` range column
  and `something` plus five blank status characters for unidentified contacts.
  The report uses the requesting unit's sensor markers even for peer-identified
  targets. Tests check formatted physical/network ranges, masked rendered names
  and status, native/Lua output, read-only state and restart. These named format
  items are implemented; they do not require another renderer.
  **TAG extended (2026-09-14):** supported vehicles now use the same selection,
  ownership, lock/recycle timers and live validity checks as Mechs. Either family
  can illuminate Mech or vehicle targets, replace another illuminator and supply
  semi-guided missile assistance. C3 masters provide integrated TAG through the
  existing computer hardware accounting. Native TAG accepts battlefield IDs or
  dbrefs and distinguishes absent equipment, destroyed equipment, invalid targets
  and the unrounded fifteen-hex range limit with reference replies. Vehicle
  inspection, status, shutdown, map membership, saved state and the simulation
  scheduler now include TAG. This does not close unrelated sensor/report gaps
  or establish every reference TAG admission/lifecycle detail.
  **Scan weapon rows implemented (2026-09-14):** both supported unit families
  now use the reference WEAPON SYSTEMS header and fixed name/number/location/
  status columns. The renderer omits destroyed sections and ammunition, preserves
  current recycle state, and uses the first critical for the public damage
  marker. Its compact display numbering does not reindex weapons in owned state.
  Tests cover all seven chassis, exact columns, first/later critical loss,
  recycle boundaries, native/Lua output and restart. The surrounding six-space
  SCAN/REPORT information block and additional INFO lines were addressed next.
  **Information rows implemented (2026-09-14):** SCAN and REPORT now share the
  reference name field, six-space indentation, tabs, coordinates/heat, full
  movement names, lateral-adjusted heading, VTOL vertical speed, turret facing,
  observer turret/weapon arcs, visible condition banners and jump heading.
  INFO adds torso and towing lines without duplicating turret facing. Owned
  status shares condition predicates and turret formatting while retaining its
  wider disclosure. All supported chassis, native/Lua, literal names, read-only
  state and restart are tested in `tests/btech_scan_summary.rs`. Unsupported
  families and separate admission details remain outside this display check.
  **C3 target-report correction (2026-09-14):** `network_targets.rs` already
  renders the separate physical `r:` and effective `c:` distances and degrades
  unidentified sightings to `something` with blank condition columns. Existing
  C3i tests verify peer-only identification, blocked sightings, native/Lua output,
  read-only state and restart; classic C3 calls the same renderer. These named
  omissions are stale. This does not claim parity for all report ordering or
  other SCAN/REPORT formatting details.

## Map and special objects

- **Buildings**: destroyed-building rebuild (7200 s, CF restored to 1) is missing and
  the `btech_building_repair` constraint `remaining BETWEEN 1 AND 120` cannot hold
  it; regen/rebuild not restarted for damaged buildings at load; step-on-base
  `THE FOO has CF of N.` notice missing.
  **Verification note (2026-09-14):** the reference's rebuild scheduler exits
  when current CF is zero, so the later rebuild branch is unreachable.
  Enabling the intended two-hour rebuild is an open behavior choice. Recovery
  of ordinary repair for surviving damaged interiors has since been implemented:
  `persistence/btech_building_repair.rs` restores missing intervals for referenced
  interiors while retaining committed clocks. `tests/btech_building_recovery.rs`
  verifies read-only loading, first-save insertion, and saved countdown recovery.
  **Step notice progress (2026-09-14):** a shared surface-entry rule now emits
  the uppercase structure name and current CF, including zero CF. Dropship
  structures are excluded; concealed structures use the existing perception/XP
  service. Ground, VTOL and jump movement call the shared rule. Tests cover five
  mobile ground chassis, duplicate entrances, hidden perception success/failure,
  and restart. The shared host boundary-exit dispatcher now also checks the
  destination after a successful transfer. All four edge directions are tested
  for biped/quad, tracked/wheeled/hover and VTOL units, including callback
  rollback and altitude suppression. Five mobile ground chassis also verify
  that an avoided uphill entry stays silent while a forced downhill fall
  reports the accepted surface hex. These tests close the named ordinary
  step-on-base notice gap; this does not establish unrelated movement parity.
- **Map load**: bad terrain char should substitute grassland and log, not reject;
  short rows tolerated; `MAPFLAG_FIRES` not set from `&` hexes, so authored eternal
  fires are erased on save.
  The `map_checkmapfile` failure replies are now exposed by `LOADMAP`: missing
  asset, invalid dimensions and incomplete rows have typed decoder/read failure
  categories and reference command wording. Native failure output includes
  `Loading <name>`; extra arguments are ignored as in the reference tokenizer.
  Dimension and row failures now publish the reference `MapErrors` diagnostic
  after restoring map state; missing files do not publish. A caught Lua rejection
  retains the diagnostic, while an aborted callback rolls it back. Channel
  publication failures roll back their state and output.
  **Verification note (2026-09-14):** authored `&` now infers flag 8 before
  explicit metadata overrides it, matching `map/map.c`; load/save/restart tests
  preserve permanent fire. The blanket short-row claim is inaccurate: the
  reference checks `strlen(row) < 2 * width` and returns -2 on premature EOF.
  The decoder now consumes bounded records (63 bytes for the header, 2001 for
  terrain rows), including line endings, and ignores bytes beyond the required
  tile pairs. Full buffers continue into the next row; embedded NUL ends the
  visible record while consuming its buffered suffix. Short rows with a newline
  in an elevation slot can pass reference preflight but abort in its checked
  coding registry. Rust returns an error for those invalid elevations instead.
  Optional metadata now reads only the first bounded post-terrain record;
  malformed records keep default conditions and inferred fire flags, valid
  records clamp gravity/temperature, and later records are ignored. File reload
  and LOADMAP now retain existing flags when metadata is absent/invalid and OR
  in authored fire; valid metadata replaces the flags, including explicit zero.
  Fresh inspection continues to start with zero flags. Map files now decode
  bytes rather than requiring whole-file UTF-8: ignored suffix bytes are accepted,
  unknown terrain bytes use grassland substitution, and invalid numeric bytes
  are rejected or trigger optional-metadata fallback. Diagnostics represent
  terrain bytes as Unicode characters. Numeric parsing now distinguishes the
  four field delimiters from the six ASCII whitespace characters permitted
  around signed integers; Unicode whitespace is not accepted as a delimiter.
  Focused parser and channel checks do not establish complete loading parity.
  Unknown terrain now substitutes grassland with preserved elevation. Native
  and Lua creation, reload and loading publish source-ordered diagnostics to
  `MapErrors` through shared transactional channel delivery; absent channels
  do not prevent loading. Inspection uses the same decoder without publishing.
- **Special-object registry**: type initialization/teardown and per-object handler
  dispatch remain incomplete. DEBUG and
  AUTOPILOT domain operations still lack complete runtime integration.
  Typed per-type command metadata and live per-object `HELP`/`HELP ALL` now
  exist. Help selects the actor, location, then persisted contents order, skipping
  Zombie candidates. It uses saved class codes and shared privilege/category
  filtering. An isolated execution of the reference dispatcher confirms that
  only uppercase HELP enters this fallback; lowercase help remains general MUX
  help. Tests cover both paths, compression settings, imported registrations,
  deliberately reversed contents order, read-only state and restart. Ordinary
  commands still use the existing global/native dispatch, so this closes the
  help route rather than the complete special-object registry.
  **Restricted admission implemented (2026-09-14):** ordinary commands now use
  the same candidate order and signed class gates before exit/native lookup.
  The first matching restricted entry emits `Sorry, that command is restricted!`
  unless the executor is GOD/Wizard; queued cause authority is not borrowed.
  A public first match ends the restriction search, preventing a later object
  from denying it. Allowed MECH/AUTOPILOT/TURRET commands still reach existing global handler
  admission and object resolution; that remaining path must be replaced with
  per-object adapters before claiming complete command dispatch parity.
  **Selected-map routing implemented (2026-09-14):** MAP matches now invoke
  existing native operations with the selected object. Nineteen map adapters
  share one target resolver; the actor is not moved to change command context.
  All 26 MAP catalogue commands have bindings. VIEW uses the map renderer, while
  STORES uses the actor-location manifest, matching the reference's overloaded
  handler. Stock correction commands also retain actor-location semantics.
  Tests compare 21 map operations from a carried map with the same operations
  from inside it, plus actor-first queued selection, an exit-name collision,
  stock exceptions and restart. Other type adapters and global fallback cleanup
  remain open alongside registration lifecycle.
  **Selected DEBUG routing implemented (2026-09-14):** all nine DEBUG entries
  now run before exit and global alias lookup. SETWBV retains the catalogue's
  public admission; general operator and Lua setters remain Wizard-only. Its
  typed mutation is shared with those setters. SETVRT/SETWBV use exact very-long
  catalogue names (including manufacturer names), reference bounds and error
  messages; wildcard and short names do not match. DEBUG SHUTDOWN requires a
  map number, silently ignores absent maps and invokes shared transactional
  map clearing for existing maps. Tests cover imported carried registration,
  exact replies, exit precedence, permissions, runtime reset and registration
  survival across restart. This closes these routing differences, not all DEBUG
  service output parity or registration lifecycle.
  **DEBUG character diagnostics corrected (2026-09-14):** SETXPLEVEL and
  XPTOP share full character-name lookup, with canonical names preceding aliases
  across values, advantages, attributes and skills. Skill abbreviation generation
  uses the same helper. Commands now distinguish unknown names from non-skill
  values and preserve reference argument/value/bounds error precedence. Threshold
  success remains silent and uses the existing audited, transactional mutation.
  Exact-message tests also verify unchanged state on rejection and a zero
  threshold selected by alias. This does not close other DEBUG service gaps.
  **DEBUG lifecycle implemented (2026-09-14):** `@btech/register thing=DEBUG`,
  `@btech/info thing` (or bare object inspection), and `@btech/unregister thing`
  now work without SQL imports. Switches accept the reference's one-character
  abbreviations and require Wizard access plus object control. Registration
  requires a live Thing, preserves containment, is idempotent for the same type,
  and rejects conflicting types. DEBUG creation/removal persists through the
  ordinary atomic world save. Other types still reject unsupported initialization
  and teardown explicitly; their domain cleanup and configuration removal remain
  open. Existing same-type registrations can be inspected and re-registered.
  **MAP initialization implemented (2026-09-14):** registration of a controlled
  live Thing as MAP uses the shared map constructor to create the reference
  21x11 grassland grid named `Default Map`. It retains initial raw gravity zero,
  temperature/flags zero, daylight, visibility 30/max 60, cloud base 200 and
  building regeneration one. Re-registration preserves the live map. Native
  registration, selected-map VIEW/LOADMAP and both initial and loaded-terrain
  restart are verified together. MAP teardown is covered below.
  **MAP teardown implemented (2026-09-14):** unregistration shares transactional
  unit shutdown and placement cleanup, then removes map-owned terrain, events,
  objects, lookup bits and configuration. Game containment and inventory remain.
  Cleanup notices go to GOD without borrowing GOD's authority for admission.
  External entrance/exit markers retain their object targets across reload;
  authored parent links are cleared. Loaders and building damage safely handle
  extant destination objects without an active MAP role. Tests cover all supported
  chassis, Going maps, external markers and reactivation, output rollback and a
  forced database deletion failure. Unsupported unit families and other special
  types' lifecycle remain outside this acceptance.
  **TURRET lifecycle implemented (2026-09-14):** registration initializes an
  unattached station with reference zero parent/gunner, unset target/x/y, and
  four zero TIC words. Both unattached and parent-attached station creation share
  the typed defaults. Unregistration removes station-owned fields, TIC rows,
  lock timers and artillery correction, preserving objects, occupants and other
  stations' state. The loader and writer now own all four required TIC records
  rather than omitting them on creation. Explicit DEBUG/MAP/TURRET role changes
  are retired before replacement in a single database save. Tests cover all nine
  final-type combinations, nonzero TIC reset, Going stations, independent clocks,
  restart and database failure.
  **Selected TURRET commands (2026-09-14):** field controls and initialization/
  deinitialization now use the selected actor/location/carried station before
  exit lookup. Five reserved TIC commands consume input silently, matching their
  empty reference handlers. Lifecycle admission accepts those candidate relations
  without moving the actor or replacing the parent pilot. Takeover checks the
  previous connected gunner against the actor's location; repeated initialization
  retains the reference joystick message. The remaining 16 turret commands still
  need selected-object adapters. MECH/AUTOPILOT lifecycle
  and same-type map replacement with opaque imported records remain open.
  **Contract characterized (2026-09-14):** candidate order, handled denials,
  signed class masks, description-based restriction markers, visible help
  categories, HELP ALL rejection for categorized types and registration teardown
  are recorded in `docs/btech-special-object-dispatch.md`. These paths require
  shared per-type metadata and existing lifecycle services; the contract document
  does not implement or close this registry gap.
- **Output**: `LIST OBJS` ordering and internal-cache marker; `LIST MECHS` diagnostics; `FIXMAP`
  repair lines; DS enter/leave map channel messages.
  **Membership span output corrected (2026-09-14):** `FIXMAP` now prints
  `Checking N entries..`, using the allocated slot span rather than the live
  unit count. `LIST MECHS` prints the reference first-free diagnostic when the
  two differ. A saved map scalar retains trailing holes: ordinary removal only
  shrinks the span when removing its final slot, and then only by one. Placement,
  transfer, scenario membership, purge and wreck retirement share the updates;
  SQLite owns the existing `first_free` column. No duplicate membership array
  is introduced. Invalid-reference repair messages still depend on reference
  states that the typed membership projection rejects; this change does not
  implement those repair branches or unsupported DropShip messages.
  **Object-table prerequisite (2026-09-14):** reference `list_mapobjs` exposes
  `X Y Type obj dc ds di`, with rows formatted as
  `%-3d %-3d %-5s %-5d %-4d %-6d %ld`. Its smoke `ds` is the original
  signed-short duration, while the expiry event has a separate countdown.
  Rust now retains `BattleDecoration.object_duration` independently of the
  countdown: smoke keeps its original signed-short duration, and fire spends
  this budget at spread events then preserves it during burnout. All marker
  creation paths share a constructor; operator commands preserve negative and
  clamped source durations independently of their scheduled expiry. SQLite
  persists the retained value with a signed-short constraint. Tests cover source
  bounds, live countdown/restart, replacement, and spread/burnout transitions.
  `dc` is underlying terrain, not remaining duration; `di` is the type-specific
  scalar. `LIST OBJS` now uses the exact header, 44-column separators and minimum
  field widths, without the extra object-count footer. Native and Lua calls share
  the typed projection. Exact rows cover active effects and imported payloads,
  including negative shorts, smoke after a tick, read-only inspection and restart.
  Active fire/smoke records now retain creation order independently of tile
  coordinates. Each new or replaced effect precedes older effects of its kind
  and imported restoration records, matching `add_mapobj` prepending. Creation
  order survives restart; invalid duplicate orders are rejected. Tests exercise
  deliberately scrambled coordinates, replacement, imported records and callback
  rollback for both effect kinds. Other object-kind ordering still needs review.
  Landing blocks now retain their own traversal order: new ADDBLOCK/native Lua
  additions prepend, explicit slot edits retain position, and removals and map
  cleanup discard matching order entries. Imported records begin in saved
  ordinal order. Mine and landing-block order persistence share one helper while
  keeping stable object IDs and extension columns. Other marker kinds remain
  open for ordering review.
  TBITS now has owned sparse packed rows, including absent, empty and zero-row
  states. LIST emits the reference information-object line; SETMAPSIZE refuses
  allocated objects before parsing dimensions. Deletion preserves definitions;
  server startup rebuilds mine coverage without rebuilding hangar bits. The
  read-only loader preserves saved bytes. Terrain reload retains allocation and
  its map-object flag. Thirty focused integration tests and two lookup unit tests
  pass, including native/Lua deletion, rollback, malformed rows, padding and
  step-notice suppression across five ground chassis. Caller acceptance and
  broader regression checks remain open in `docs/btech-map-bits.md`.
  Imported fire/smoke/generic restoration records now retain `object_dbref`,
  `data_short` and `data_int` in the typed model as well as the database.
  Copying a record to a new ordinal preserves its payload instead of replacing
  it with insertion defaults. These imported durations do not create timers.
  Building entrances and return links now own their authored byte/short/scalar
  payloads; arrival points retain their object reference and short/scalar values.
  Persistence and copied records keep those values, and destination-only edits
  retain the existing return-link metadata. Tests import nonzero payloads and
  verify signed limits, copies, coordinate edits and restart. Linked markers now
  own their complete payload; coordinate-only edits retain it. Landing blocks
  also own their authored signed-short field. Both are projected into the table.
  Lua map inspection exposes linked markers as records with `coordinate`,
  `object`, `data_char`, `data_short` and `data_int` fields.
  **Marker command replies corrected (2026-09-14):** `ADDFIRE` and `ADDSMOKE`
  consume three space/tab-delimited fields, ignore trailing arguments and use
  the reference's command-specific missing-field/numeric-error replies.
  Shared parsing accepts signed 32-bit integers and rejects overflow. Focused
  tests cover both commands, each numeric field, signed zero, trailing arguments,
  rejected-input state preservation and existing duration/restart behavior.
  **LIST admission corrected (2026-09-14):** target names are exact,
  case-insensitive `MECHS`/`OBJS`, as required by the reference `listmatch`.
  Native parsing ignores arguments after the first target token. Invalid-target
  replies preserve the submitted token's casing. Lua uses the same target-name
  validator. Listing tests cover abbreviated-name rejection, mixed-case names,
  ignored native suffixes, read-only state and output rollback. This does not
  close object ordering or invalid-unit diagnostics listed above.
  Native LIST now splits only on space/tab, retaining Unicode whitespace inside
  invalid target tokens. `ADDBLOCK` likewise consumes at most four space/tab
  fields and ignores later arguments. Missing fields, malformed signed 32-bit
  numbers and invalid coordinates now use the reference replies and validation
  order. Tests cover every numeric field, overflow, signed zero, optional teams,
  ignored suffixes, Unicode whitespace rejection and native/Lua state agreement.
  `SETCOND` now uses the reference's field-specific error wording and validation
  order, including signed-zero acceptance. Its tokenizer consumes at most four
  fields, so additional arguments are ignored; the reference's too-many-options
  branch is unreachable. Typed Lua updates continue through the shared setter.
- Fire spread, ice, wind, links, minefields, LZ blocks, environment: verified
  equivalent.

## UI and output

- **Tactical**: DropShip footprint (`X`, `@`, `=`) and MW `btech_mw_losmap`
  forcing remain open with their unit families. **Audit correction:** LZ glyphs
  already match the executable reference: suitable is green `O`, unsuitable red
  `X`. `aero_landing_zone_check` returns zero for suitability; misleading local
  marker names do not invert that return value. Dark-map cliff/LZ guards match
  supported classes, and both tactical dimensions already clamp to twice the
  tactical range. Invalid single-letter flags now use the reference reply.
  Other tactical admission/error and unsupported-family behavior still need review.
- **LRS**: glyphs `d a n s f` remain open with their unit families.
  Stacked markers now follow reference positional exchange ordering of visible
  viewport occupants; self has no special priority. Native and Lua displays use
  the same read-only selection and preserve membership order across restart.
  Coordinate headings now retain the reference's fixed three rows, including
  first-three-digit truncation above 999. **Audit correction:** odd-height
  forcing already matches the reference: clamp the requested height to twice
  the sensor range, make it odd, then clip visible rows to map bounds. A short
  even-height map therefore still displays an even number of hex rows.
- **Status**: MW and BattleSuit blocks; movement types Flight/Hull/Submarine/
  Hydrofoil; `Max thrust`; `Fuel: Unlimited`; remaining condition/technology
  banners. Mech `ChargeTarget`/`ChargeTimer`, action-time STAGGERING,
  TRACK/AXLE/LIFT FAN/ROTOR DESTROYED and ignored unknown selector letters are
  implemented. Shared layout and native/Lua tests are in `tests/btech_status.rs`;
  those checks do not close unsupported-family or remaining banner gaps.
  **ECM status color corrected (2026-09-14):** enabled Guardian and Angel ECM
  now use the committed countered flag for their red/green indicator, matching
  `mech_condition_summary`, rather than testing reactor power. Both chassis
  stores share this presentation rule. Tests cover Off/ECM/ECCM colors, native
  and Lua reports, unchanged field state and restart; the live electronics
  scenario verifies green emission, enemy ECCM countering to red and destroyed
  equipment overriding the mode with `XX`.
- **Armor diagram**: MW/BSuit/aero/DS/foil/ship/sub templates; adversarial `OoxX*?`
  mode with Key legend; `DIVIDE_10`/`SHOW_DEST` flags; per-player Lua template.
  **Standard adversarial diagrams implemented (2026-09-14):** ordinary scans
  now reuse the eight supported cockpit silhouettes, typed protection cells
  and section masks. The shared integer bands supply both colors and `OoxX*`
  fills, with the reference three-column legend and chassis-specific legend row
  positions. Owned status and privileged observer scans retain numeric values.
  Native/Lua, restart, disclosure and all eight silhouettes are verified in
  `tests/btech_scan_diagrams.rs`; existing cockpit snapshots remain unchanged.
  Repair `?`, unsupported-family flags and custom templates remain open.
- **Weapon table**: vehicle charge status line; `PersonalECM`, `Carrier:`,
  `AdvItems:`, `Special Actions:` lines. MASC and supercharger counters are
  displayed with the reference color thresholds, after TAG. Physical-weapon
  readiness/recycle entries (`Axe[LA]` and other installed equipment) now use the
  shared status renderer and have reference-order and native/Lua tests.
- **XP leaderboard menu corrected (2026-09-14):** `XPTOP` now uses shared
  39-column cells and blue 78-column separators. Its optional total occupies one
  cell, matching the reference partial row. Native and Lua reports share the
  rendering, with literal player names escaped before width-aware truncation.
  Ranking now preserves the reference's immediate exchanges when a later player
  has a strictly greater balance; displaced ties can differ from a stable sort.
  Exact rows and a three-player displaced-tie case are covered alongside the
  existing restart, top-sixteen, player-cap and rollback tests. Zero-total
  percentages remain finite and aggregate totals remain overflow-safe; these
  are retained numerical differences, not claims of exact reference arithmetic.
- **`weaponspecs`**: now uses the reference fixed-width green header, blue
  78-column rules and centered model title through a shared read-only menu renderer.
  Extended-range columns and configured recycle values remain live catalogue
  queries; MML keeps both functional ammunition profiles. Interactive/multi-column
  **Coolmenu** features remain open. **Dice-report inventory correction:**
  the reference registers `do_show_stat` as wizard-only `+rolls`, not `@stat`.
  The report reads only the generic `btech_random_roll` histogram; direct
  character d6 rolls and separate hit/critical diagnostic histograms are not
  interchangeable with it. `+rolls` now exposes the read-only aggregate with
  wizard admission, ignored trailing arguments and reference switch rejection;
  see `docs/btech-roll-statistics.md`. The typed histogram and exact
  report renderer now exist with unit coverage. Explicit per-stream journals now
  classify ordinary attack and caseless checks separately from direct three-die
  attacks. Piloting, orbital landing, boosters, ordinary heat checks, woodland
  effects, fire spread, analog radio distortion, pod swatting and searchlight
  damage now also record their generic checks; direct effect dice and the
  Computer skill override remain excluded. Combat location/critical rolls,
  physical attacks, ordinary clusters, swarm continuation, Clan AMS, immobile
  aiming, sixth sense, perception and RAC unjamming are also explicitly classified.
  Hotloaded clusters, IS AMS and targeting-computer d6 remain direct draws.
  World-level totals now combine live journals with history retained at database
  purge and wreck retirement, including maintenance callback replay. Command,
  lifecycle and restart tests verify active rule accounting; repair and unsupported
  unit-family rolls remain with their deferred systems.
- **Base entry**: prone/standing, jump, uncontrolled-flight, airborne VTOL and
  excess-argument replies now use reference wording through shared admission.
  Other refusal precedence and route/lock diagnostic parity still need review.
- **Radio audit correction**: `Are you trying to kid me?` is unreachable in the
  reference because its digit-only parser first rejects negative input as
  `Invalid frequency!`. Rust retains that reply and now also rejects leading
  plus signs and signed-integer overflow with the same lexical error. Values
  within the parser domain but above 999999 retain the separate range error.
- **Map-display bounds verified (2026-09-14):** the Rust-only `mapdisplay`
  command uses the exact limits in the reference's
  `btech_player_ui_preferences_set`: width 5–40, tactical height 5–24 and LRS
  height 10–40. Defaults also match: 21 by 14, LRS height 11. The native test now
  covers all eight min/max combinations and rejects both sides of each range,
  while retaining the existing player isolation, rollback and restart checks.
  These are requested dimensions, before sensor-range and map-edge clipping.

## Character and experience

Skills (78/78), attributes, XP arithmetic, consciousness ladder, head-hit injury,
gunnery/piloting/comm/perception/spot/artillery XP are ported. Configured sprint
myomer policy now reaches classic and battle-value gunnery awards for both
participants and the `bv` field. Boundary and replay checks are recorded in
`docs/btech-sprint.md`; this does not close remaining movement-policy consumers.
Gaps:

- **Chargen** (`cm_*`, `chargen_*`, levels/types/packages, career flags): absent.
  **Reference-source qualification (2026-09-14):** the named `cm_*` and
  `chargen_*` entry points occur as declarations in `character/btechstats_api.h`;
  no implementations or callers were found in the reference C sources or Lua
  scripts. These declarations do not establish executable character-generation
  rules to reproduce. A character-generation workflow remains unimplemented,
  but its levels/packages and progression need a defined behavior contract;
  it cannot be verified by treating those header declarations as working code.
- **Advantages** untyped; 17 of 22 have no effect; `Exceptional_Attribute` to `Lives`
  coupling lost.
  **Typed catalog and boolean consumers corrected (2026-09-14):** all 22
  advantages now have shared canonical names and boolean/ranked/attribute-mask
  classifications, also available through `character.advantages` in Lua.
  Existing injury, falling, physical combat and movement consumers now share
  case-insensitive, exact-one boolean interpretation. Several previously used
  positive-value checks and exact-case keys, giving values 2–255 effects that
  the reference boolean helper does not grant. Raw stored values and their
  experience/last-use metadata remain unchanged. This does not implement the
  remaining action-specific effects or character generation.
  **Life-accounting claim corrected:** `char_setstatvalue_by_code` adjusts Lives
  only when the numeric code equals `EE_NUMBER` (11). In the current reference
  catalog, index 11 is ShotsHit; Exceptional_Attribute is index 20 and Extra_Edge
  is index 21. Thus the reported Exceptional_Attribute coupling is not executable
  reference behavior. No life-accounting side effect was added. Intended
  Extra_Edge behavior and the stale numeric coupling require separate review.
- **Pilot startup health added (2026-09-14):**
  the tactical injury/consciousness ladder already exists. Startup now shares
  `fix_pilotdamage` arithmetic across Mechs and vehicles: health sum divided by
  twice Build, with divisor 10 for invalid Build and signed-byte saturation.
  Confirmed tactical crew death is stored separately from the injury count, so
  a high health-derived count does not destroy the unit merely by starting it.
  Later tactical injury still kills at six or more; IC injuries retain their
  independent health/death calculation. Startup preserves health, dice and
  recovery timing. Native/Lua, rollback, all seven chassis, both IC modes and
  restart pass focused tests. The completed broad baseline exposed stale wreck
  fixtures and a reactor expectation; all affected suites now pass their
  corrected checks. A separate movement/crew/field run passes 429 tests.
  **Counter consistency corrected:** startup, character damage, tactical damage
  and administrative edits now share one count writer for both chassis stores.
  `pilotdam`, cockpit status and character-pilot reports agree after injury and
  clearing. Confirmed death remains independent. Load validation rejects counts
  above 127. Wreck fixtures now set the explicit death marker; a six-count live
  startup is not a wreck. `initialize_pc` and the MW ejection caller remain
  unimplemented.
- **Personal combat** (`pcombat.c` 14-entry armor table): absent; 9
  `btech_player_configuration` loadout columns are never read.
- Computer XP from overheat is now implemented: successful player overrides in
  IC units use the shared skill threshold and cooldown and publish accepted
  one-point awards to MechXP within the thermal transaction. Failed, absent or
  non-IC overrides do not award. Tech XP remains deferred.
- **Evacuation XP inventory corrected (2026-09-14):** the shared Rust
  evacuation path already calls `retain_character_experience` after successful
  movement for non-wizards, gated by IC configuration and `xploss < 1000`.
  The reduction now follows the reference state adapter's selected entries:
  skills, advantages and Lives. Unselected/custom saved entries are preserved
  rather than aborting evacuation; signed-negative XP clears to zero. Skill
  bonuses recalculate using runtime thresholds; non-skill selected values retain
  their reduced balance with no bonus under their default zero thresholds.
  Tests cover every advantage and skill, floor arithmetic, restart, wizard
  exemption, lethal head damage and failed movement/callback rollback.
  This establishes the casualty-evacuation hook; MW autoejection remains separate.
- **Recovery retry feedback added (2026-09-14):** player recovery reports
  carry the already-resolved target/roll and cockpit audience. Occupied cockpits
  receive the two private reference attempt/roll messages, followed on success
  by `The pilot regains consciousness!` to occupants. Failed checks do not
  broadcast. Blinded cockpits suppress feedback without changing dice or timing.
  Empty crews announce successful recovery only. Existing player-owned recovery
  after cockpit release retains private outcome feedback. Tests cover literal
  wording, real notification fan-out, all seven chassis, success/failure,
  blindness, absence of extra dice and save-failure replay.
  `mwlethaldam` still has no live C caller; lifecycle differences such as
  player-owned recovery after cockpit release remain distinct from output parity.
- Radio/computer brand failure families and `mech_rrec_event`/`mech_srec_event`
  recovery timers. The shared computer selection core now covers the rare gate,
  actual quality lookup, one-time six reroll and display/target prerequisites;
  it now runs at the live turn boundary, with shared target-loss and shutdown
  effects and durable, ordered sensor recovery events across both chassis stores.
  Restart, transaction rollback, per-display query admission, preserved contacts,
  destroyed recovery recipients, object purge and moving/airborne shutdown are
  covered. Reference ordering places computer checks after stagger handling.
  **Radio inventory correction:** the only reference generic-failure caller
  selects the computer; radio handlers and their recovery timer have no active
  caller. They remain unused source inventory, not an enabled heartbeat behavior.
  See `docs/btech-computer-failures.md` for the evidence and behavioral contract.
- **Combat statistics clarification (2026-09-14):** the character catalogue
  defines ShotsFired/Hit/Missed and DamageTaken/Given, but no reference combat
  caller writes those character values. Actual counters belong to each unit's
  runtime state (`mech_progress_state.c`, `mech_damage_history.c`). The rewrite
  now stores shared unit `shots_fired`, `shots_hit` and `shots_missed` values,
  exposes their signed field controls and updates them once per launched direct
  unit attack. Coordinate/observer fire and failed Streak/loader launches do not
  count; out-of-range ordinary launches count as misses. Broadcast glancing
  classification precedes missile grouping and defenses. Both chassis stores,
  native/Lua, persistence and rollback use the same counter rules. Remaining
  work includes complete kill-event and damage-attribution acceptance;
  do not invent per-character combat writes to satisfy the original stale claim.
  **Damage entry accounting added (2026-09-14):** Mech and vehicle packet owners
  now share `damage_taken`/`damage_inflicted` storage, field controls and checked
  updates. Counters use admitted incoming packets rather than armor removed,
  skip combat-safe/cocoon interception, apply rotor scaling before accounting,
  and avoid duplicate armor/internal or section-transfer counts. Five new tests
  cover mixed chassis, overkill, immunity, material modifiers, standalone
  internal damage, destroyed-limb redirection, rollback and restart. Additional
  checks verify normal/glancing kick attribution, self-attributed balance falls,
  a laser-triggered ammunition explosion and common reactor-blast attribution
  across all supported chassis. Other physical attacks, vehicle ammunition
  cascades and delayed-blast attribution still need acceptance coverage; see
  `docs/btech-unit-statistics.md`.
  **Kill transition accounting added (2026-09-14):** shared signed
  `units_killed` storage and field controls now record first destruction at
  material and lethal critical/crew events, before nested consequences. Self
  damage awards no kill; counter overflow rolls back the attack. Mixed-chassis
  lethal shots, signed edits, native/Lua, restart and rollback pass, and the
  seeded ammunition cascade awards exactly once. Mech vacuum exposure now
  forwards the attacking unit to shared section disablement; water flooding
  remains self-attributed, as in `mech_flood_section`. The exposure matrix covers
  both Mech chassis, surviving head structure, restart and rollback. The
  previously reported ground-vehicle vacuum mortality gap was incorrect:
  `mech_parts_destroy` returns from its vehicle equipment branch before the
  later ground-vehicle death branch. All five vehicle families retain equipment
  disablement and zero kill credit, with an explicit shot/restart/rollback test
  matrix. Reactor attribution now has direct-shot acceptance for both shooter
  stores and Mech chassis: a neighboring vehicle dies in the blast, but only the
  initiating reactor destruction credits the shooter. Standalone blasts award
  no kills. Native/Lua, restart and callback rollback agree. Exhaustive lethal
  critical/crew branch acceptance remains open.
  **Initial inferno attribution corrected (2026-09-14):** vehicle missile
  exposure now retains its shooter through standard heat explosions and
  advanced initial section-fire packets. Damage and first-destruction credit
  use the shared counters; subsequent fire pulses remain self-attributed.
  The new native/Lua matrix covers both shooter stores, all five vehicle
  families, both fire policies, restart and callback/counter-overflow rollback,
  including a rotor-triggered power-plant catastrophe that awards exactly once.
- **`+charclear` added (2026-09-14):** wizard-only player lookup and reset
  clears skills, advantages, XP and last-use timestamps, with health zero and
  attributes one. An explicit default profile represents the reference's
  absent-state read defaults, keeping assigned pilots and pending recovery valid.
  Unit injuries, assignment, recovery countdown and dice are preserved; cleared
  recovery advantages no longer affect later rolls. Tests cover denied/invalid
  requests, repeated clears, absent profiles, restart, exact next recovery rolls,
  and subsequent injuries for both Mech and vehicle pilots.
  **`+show` added (2026-09-14):** wizard-only
  character catalogs preserve headers, three-column spacing, totals and catalog
  order through shared skill/advantage definitions. `btechvalues` includes all
  115 reference field names and object-kind numbers; field setter implementation
  remains separate acceptance work. Full category names are case-insensitive;
  abbreviations and advertised `char_` prefixes are rejected by the actual
  reference lookup. The second equals-separated argument is ignored.
  **Character-list API added (2026-09-14):** `btech.character.list(kind, player?)`
  implements the `btcharlist` category query using existing catalogs. It accepts
  full case-insensitive category names and optional player IDs, names, aliases or #dbrefs;
  abbreviations and explicit nil are rejected. Skills are filtered by nonzero saved value or XP.
  The reference condition does not filter advantages, so advantages and
  attributes remain complete even with a target. Results are detached and
  read-only; malformed calls, absent character profiles and restart are tested.
  **Threshold inventory corrected:** `btech.character.threshold` and
  `set_threshold` already provide runtime skill-threshold inspection and wizard
  edits, verified by `tests/btech_character.rs`; the Lua API raises an error for
  unknown skills rather than returning the old softcode `-1` sentinel.

## Economy (deferred)

Stores, manifest, cargo load/unload, stock wizard commands are ported. Missing:
`btech_economy_costs` read/write, `btgetpartcost`/`btsetpartcost`, `mech_fasa_cost`
unit cost, brand-collapse and actuator aliasing in `econ_find_items`.

## Persistence and schema

The 43 C tables exist in Rust with identical columns, types and constraints. Rust
adds 12 JSON-blob tables (`btech_units`, `btech_vehicles`, `btech_tows`, ...).
Behavioral gaps:

- The 13 `btech_mech*` column tables (incl. 95-column `btech_mech_runtime`) are
  purge-only. A Rust save read by C loses all unit state. Decide: write them for
  interop, or accept the blob tables as the schema of record.
- `btech_unit_configuration` (preferred_id, display_name, markings, assigned_pilot)
  never read or written.
- `btech_economy_costs` never written (C creates it inside the save transaction).
- `btech_repair_events` and 4 autopilot tables never restored (deferred).
- `btech_player_configuration`: 14 of 24 columns unused.

## Deferred subsystem inventories

**Repair** (~9k lines): job engine with tech skill/time/roll and 15 event kinds;
armor/internal/section/gun/part/ammo jobs; scrap/salvage; `FIX`/`DAMAGES` planner;
`REPAIRS` listing and `btech_limitedrepairs` policy; 30 `@mech/<switch>` admin
switches (`/loadnew /restore /addweap /setarmor ...`). Present already: building
repair, turret unjam, parts store, skills, criticals model. Gate item: job engine +
tech-time ledger.

**Autopilot** (~11k lines): AUTOPILOT special object, 100-slot order queue, 17 typed
orders, A* pathfinding with terrain-cost policy, goto/follow/roam/chase/base
executors, autogun with target scoring and weapon profile, physical attacks, sensor
policy, radio command parser (28 verbs), AI heartbeat. Schema already present.

**Unit classes**: naval (hull/foil/sub), aerospace, both DropShip classes, ejected
MechWarrior, battle armor, infantry. MW is the one that leaks into in-scope play
(autoeject, hit/LOS special cases, `+2` to-hit, `MechWarrior:` status block).

## Prioritized implementation plan

Revised 2026-09-14. Original items 1, 2, 6, 7 and 9 are complete for supported
Mech/ground-vehicle/VTOL play, or were closed by reference tracing; their leftover
pieces are folded into the items below. Ordered by player-visible impact, then
fidelity, then deferred scope.

1. **Shared command admission layer.** Add one `common_checks` equivalent run at
   command entry with the C texts (`You are destroyed!`, `Reactor is not online!`,
   `You are unconscious....zzzzzzz`, `Now now, only the pilot can push that
   button.`, invalid-map auto shutdown + index reset) and the `last_use` reset.
   Replace the three divergent replies in `power.rs`. The per-class restricted
   gate already exists in `special_dispatch.rs`; route ordinary handlers through
   it so dispatch no longer falls back to global lookup.
2. **Mech chassis parity.** Extend the Mech validator and rules to ICE,
   Waterproof and the TargComp chassis flag (vehicle rules exist to share);
   OmniMech + `OMNI_BASE_MODE`; ForceSingleHS; NoSensors; make `standanyway`
   consumed by `stand.rs`. Unblocks templates that fail construction today.
3. **Ejected MechWarrior class.** `CLASS_MW` unit type built by `initialize_pc`
   from the pilot template; MW hit/LOS/to-hit/detection special cases, status
   block and armor diagram, fire-hex fainting; then autoeject from section
   destruction and reactor explosion, `embark`/`disembark`/`udisembark`,
   `pickup_mw`, and `attachcables`/`detachcables` (verified MW-only actions). The
   personal-combat armor table (item 6) supplies MW armor.
4. **Remaining sensor items.** `perecm`/`pereccm` (state and messages exist,
   mode is hardcoded Off); `MOVE_NONE` stationary Mech representation for the
   140% reach; post-detection perception XP award.
5. **Behavior decisions to record, then implement or close.** Mech water-depth
   MP penalty (dead in live reference); destroyed-building two-hour rebuild
   (unreachable in reference; needs the `btech_building_repair` constraint
   widened if enabled); xoshiro256** vs ChaCha8 seed-replay parity. Each needs a
   one-paragraph decision in this document before code changes.
6. **Character completeness.** Personal-combat armor table so the nine
   `btech_player_configuration` loadout columns are live; `Extra_Edge` review;
   chargen only after a behavior contract is written (the reference has
   declarations, no implementation).
7. **Output fidelity leftovers (supported families).** `PersonalECM` and
   `Carrier:` technology lines; per-player Lua status template override; repair
   `?` armor fill; movement message text drift where not yet aligned; a
   multi-column interactive coolmenu if `HELP ALL`/`weaponspecs` need it.
8. **Scripting parity decision.** Either add the C data/admin Lua API alongside
   the command mirror (`template.*`, unit getters, `map.line_of_sight`/
   `blast_zones`/`units`, parts catalog queries, constant namespaces,
   `character.catalog`, `unit.assigned_pilot`, `player.loadout`) or document the
   surface as intentionally different with a migration note. The 67 `bt*()`
   softcode functions need the same decision; `bthexlos`, `btlosm2m`,
   `btlistblz`, `btmakepilotroll` still have no substitute.
9. **Core runtime.** Add the 14 missing diagnostic channels; give `EVENTSTATS`
   a meaningful per-kind count or drop it.
10. **Persistence interop.** Decide whether the 13 `btech_mech*` column tables
    must be written for C interop; read/write `btech_unit_configuration` (markings
    now live in the blob) and `btech_economy_costs`; wire remaining
    `btech_player_configuration` columns.
11. **Special-object registry.** `@btech/register|unregister` with the C replies,
    per-object handler adapters replacing global fallback, DEBUG object type.
    Prerequisite for AUTOPILOT objects.
12. **Deferred: repair.** Job engine and tech-time ledger first, then structure,
    section, part/gun, ammo, scrap jobs, `FIX`/`DAMAGES`/`REPAIRS`, parts
    consumption, `@mech/<switch>` admin surface, `repair.*` Lua,
    `btech_repair_events` restore.
13. **Deferred: autopilot.** Order queue, typed API and persistence first (schema
    and command metadata exist), then A* and terrain cost policy, goal executors,
    autogun and target scoring, radio parser, AI heartbeat.
14. **Deferred: economy costs and unit classes.** Part/unit cost; naval,
    aerospace, DropShip, battle armor, infantry (hit tables, armor diagrams,
    status blocks, LRS/tactical glyphs, DS map messages and commands).
