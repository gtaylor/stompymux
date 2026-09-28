# BattleTech coverage and delivery inventory

For current scope, validation and follow-ups, see
[the first implementation summary](btech-first-implementation.md).
The detailed entries below include historical milestones.

The current integration scope excludes autopilot and game repair systems, naval
units, aerospace units and dropships, infantry, and battle armor. Their remaining
behavior is deferred; existing coverage is retained in this inventory. Mechs,
ground vehicles and VTOLs remain in scope.

Loading a unit into another unit, unloading it, and container cargo handling are
deferred. External towing remains in scope. Existing transport-loss coverage is
retained; outstanding container transport work does not block this phase.

The target is native Rust application code, existing game assets and combat rules,
and simpler interfaces where beneficial. C is a read-only behavioral reference;
there is no interim FFI runtime. LuaJIT and SQLite are outside the rewrite scope.

Counts below describe the inspected reference checkout: 550 C/header files,
125,733 lines under `src/btech`, 17 C files in the MUX btech Lua package, and 91
C files under `tests/unit/btech` (including test support). They are an inventory,
not a percentage-complete estimate.

## Initial subsystem inventory

This table records the initial migration assessment. Its coverage and remaining-
behavior columns are historical, not the current acceptance status. The subsequent
entries record implemented behavior and its verification; an earlier open item
may be closed by a later entry. Full completion requires reconciling those entries
against the reference behavior, not treating this table or passing tests alone
as proof.

| Reference subsystem | C/header files | Rust coverage | Remaining behavior |
| --- | ---: | --- | --- |
| core / include | 27 | Domain/adapter boundary, transactional one-second tick and persistent unit dice | Context configuration, roll statistics, remaining events and heartbeat systems, channels |
| unit | 107 | Biped asset decoder, initial typed equipment/loadouts and persistent construction, placement and pilots; saved identity inspection | Remaining equipment catalogs, complete chassis validation, live simulation, loadouts, state operations |
| map | 36 | Source decoding, persistent dictionaries, explicit creation/reload, occupancy, placement and normalized geometry | Full lifecycle, hazard traversal, links, bridges, buildings, environment, overlays |
| commands | 9 | Wizard inspection/map creation/reload; pilot, power, motion, torso and arm controls | Contextual unit/map dispatch, operator controls, restrictions, discovery |
| persistence | 19 | Owned unit construction, identity projection, map writes/dictionaries and purge reconciliation | Unit runtime transitions and remaining map state, durable jobs, restart normalization |
| movement | 59 | Level-ground continuous motion, acceleration, turning modes, controls, saved map movement rates, segment traversal, damage-adjusted mobility, basic terrain speed, persistent heat, thermal speed penalties, water/fire/temperature rates, ground/water falls, persistent flooding, critical/leg-loss balance, standing, prone controls/fire support and damage-induced stagger | Elevation/hazard transitions, exact boundary parity, jumping, ice/bridge and vertical falls, heat hazards and remaining environment, damage/boosters, towing, flight |
| sensors | 33 | Geometric range/bearing, standing-ground terrain LOS, saved battlefield light/visibility, visual/light-amplification eligibility and aim contributions, replayable optical detection rolls and ordered primary/secondary scans and durable optical selection countdowns and persistent per-unit contact transitions and automatic tactical scanner ticks with saved signatures/startup perception | Complete LOS, contact acquisition, scanning, targeting, sensor selection and conditions, C3/C3i, ECM, TAG |
| combat | 78 | Biped location tables, hit arcs, critical eligibility and head-hit variants, persistent material damage phases, slot selection/loss and section/core/system kills and atomic conventional hit/explosion cascades and grouped tactical weapon salvos with pilot injury/stun, ammunition expenditure, committed recycle timers and cockpit stun; conventional range/movement/heat/equipment aim contributions and biped mount arcs | Firing, ammunition, hit locations, damage, criticals, physical attacks, artillery, mines, ejection |
| character | 17 | Persistent health/attribute records, detached inspection, cockpit injury arithmetic, consciousness rolls and player-owned recovery timers and tactical cockpit injury/unit loss | Full casualty lifecycle, skills, personal combat, XP, battle value |
| economy | 7 | Deferred | Parts, cargo, costs and stores |
| repair | 35 | Deferred | Repair planning, construction, salvage, execution, timing, restore, administration |
| autopilot | 52 | Deferred | Orders, pathfinding, movement/combat policy, radio, autogun, persistence |
| scripting | 20 | Inspection, map creation/reload and tile queries | Complete domain query/mutation API and script results |
| special | 11 | Saved registration inspection | Special-object creation, lifecycle, dispatch and help |
| ui | 35 | Wizard inspection, piloting/sensor controls and current acquired-contact display | Status, tactical/LRS maps, contacts, radio, broadcasts, styled notifications |
| integration | 5 | Read-only template construction diagnostics, rejection reasons and ammunition normalization previews | Remaining operator diagnostics and debug command catalog |

The domain inventory must also include `src/mux/lua/packages/btech`, MUX command
dispatch, world movement/destruction, server heartbeat/startup, configuration
catalogs, help, Lua type definitions, and all btech relational tables. The source
directory alone does not describe the complete extension boundary.

## Ordered acceptance gates

1. **Contracts and inspection — initial implementation delivered.** Bounded fixture
   parsing, read-only identity loading, command/Lua adapters, and preservation tests.
   Remaining characterization includes numeric rules, timing, notification routing
   and paired reference execution.
2. **Persistent maps and units — initial milestone delivered.** Terrain dictionaries,
   explicit creation/reload, equipment resolution, unit construction, placement
   and pilot assignment are implemented. `tests/btech_crew.rs` creates two JR7-D
   units, enters them, claims their cockpits, inspects/restarts state and leaves.
   Full chassis validation, remaining equipment and unit classes remain in later gates.
3. **Mech duel — partial implementation, not delivered.** Startup/shutdown, ground
   motion, persistent dice, optical contacts, target locks, native/Lua direct fire,
   supported lasers/missiles/AC20, heat hazards, damage/criticals, ground/water falls,
   standing, tactical pilot injury and unit destruction are implemented. Basic
   dry-terrain, water, ice and bridge jumping now include durable flight, landing, stabilization and
   native/Lua controls and early landing. Airborne firing, gyro/jet damage and structural collapse
   during flight have seeded coverage and native/Lua parity tests.
   Hill collisions and elevation changes during jumps are supported. Atomic ice fracture and
   neighboring-unit falls are integrated; automatic ice triggers
   are now connected for landings and falls on intact ice. Optical LOS, automatic
   acquisition and native/Lua fire are tested on ice surfaces and during ice
   jumps, including restart and callback rollback. Native/Lua standing on intact
   ice includes failed-attempt fractures, neighbor effects and durable rise/retry
   timers. Additional terrain interactions remain pending. Bridge collapse now shares the atomic
   terrain-break resolver, including the reference’s replacement-depth selection
   and persisted altitude for occupants that do not fall. Bridge deck LOS and
   optical contacts include native/Lua queries and collapse/restart refresh.
   Bridge falls select deck or depth-one lower surface, with native/Lua standing
   and direct fire, cooling and breach effects. Bridge jumps include entry and
   underside collisions, retained positions through interrupted hex updates,
   and native/Lua launch and landing. Level bridge decks and adjoining land
   support forward/reverse ground travel with midpoint restart. Forward one- and
   two-level steps apply elevation speed costs on land and bridge decks. Cliff transitions now use speed-dependent checks, uphill collisions and downhill falls, including the skid-rule variant. AutoFall is persisted and exposed through native/Lua controls; it skips piloted downhill avoidance. Ground water movement now applies hex-entry control checks, forward throttle limits and flooding, with replay and atomic failure coverage. Below-deck ground travel now preserves selected altitude, water checks and mapped cliff/step rules, including rollback and restart. Ice ground travel includes entry fracture, neighboring falls, bottom movement, depth-one surfacing and submerged standing. Airborne ice crossings now include upward breakout, downward falls, interrupted horizontal entry and destination-landing ordering, with replay and rollback tests. High-water ground entry and jumps now preserve mapped heights, with special entry checks and wet fall damage. Native/Lua controls reject running in high water or submerged water/ice/bridge terrain. Reverse one- and two-level steps now use the configured piloting checks, falls and uphill rollback. Automatic collapse
   triggers remain pending. Building/wall ground routes and jumps use shared mapped-height rules, with slope/cliff, collision, native/Lua and replay coverage. Live fire/smoke ground crossings are supported. Remaining jump combat interactions,
   remaining terrain transitions and combat notification/interface coverage remain.
   `tests/btech_duel.rs` now runs two live clients through cockpit entry, normal
   startup, optical acquisition, settled locks and mutual fire until destruction,
   then restarts the server and verifies that the wreck cannot start or fire.
   A shared crowding resolver now covers occupancy, team selection, collision damage
   and avoidance with explicit physical inputs. Automatic crowding hooks
   now use calculated current mass for ground entries, normal/early landings, jump-obstacle crashes and airborne thermal
   shutdown falls. Airborne critical-damage falls also resolve crowding inside the damage transaction;
   moving ground shutdown also applies configured fall crowding. Airborne voluntary
   shutdown now schedules durable free fall, including terrain contact, atomic
   damage, native/Lua commands and server ticking. Aircraft recovery and remaining
   environment lifecycle hooks are pending.
   Mass inspection includes component accounting, loss and restart tests.
   This complements the deterministic rule and atomic-failure tests; it does not
   close the remaining movement and interface requirements of this gate.
4. **Equipment and rule coverage — not delivered.** Conventional small/large lasers
   and PPCs now share the firing pipeline and support the existing AWS-8Q and HBK-4P
   assets, with native/Lua, damage, heat and recycle replay tests. AC/2, AC/5 and
   AC/10 also use the shared firing path, including ammunition and bin hazards;
   the existing ENF-4R asset constructs. SRM-2 and LRM-5/10/15 now include
   cluster/glancing rules, native/Lua firing and saved replay; existing GRF-1N,
   CPLT-C1 and TBT-5N assets construct. Ordinary machine-gun/flamer fire is
   supported, including the existing FS9-H and LCT-1V assets and persisted
   case-insensitive arm-flipping controls. Flamer heat mode now supports template
   initialization, persisted native/Lua toggles, ordered native lists/ranges and
   heat-only shot effects. IS ER lasers/PPCs, pulse/X-pulse lasers and light/heavy
   PPCs now share firing and restart handling, including pulse accuracy and
   individual heat/range/recycle values. Snub-Nosed PPCs now use live spatial
   range for damage before glancing reduction, with native/Lua threshold and
   restart coverage. IS double heat sinks now support grouped critical losses,
   flooding, cooling and mass, including unchanged BJ-3 and APL-1R assets.
   Distributed IS Ferro-Fibrous (standard/light/heavy) and Endo Steel now
   derive mass benefits from installed slots and exclude material slots from
   random critical hits; CRB-28 constructs and replays through storage/Lua.
   Canonical IS light/XL/XXL/compact fusion engines now derive from slot layout
   with mass, engine-hit heat, torso-loss destruction and restart coverage; AF1
   constructs with an inferred XL engine. Installed CASE now contains internal
   explosion transfer, with local destruction, crew injury, XL loss, native/Lua
   fire and restart coverage; HBK-5M constructs unchanged. IS Streak SRM-2/4/6
   now include failed-lock recycle without heat/ammunition, full missile hits,
   glancing boundaries and native/Lua replay; BJ-2 constructs unchanged.
   IS MRM-10/20/30/40 include their accuracy penalty, distinct cluster tables,
   five-point damage groups, glancing rules and native/Lua recycle replay;
   QKD-8K constructs unchanged.
   IS ELRM-5/10/15/20 now include extended ranges, distinct cluster tables and
   lowest-two-of-three attack rolls below minimum range, with native/Lua firing
   and saved recycle coverage.
   IS long-range dead-fire 5/10/15/20 and short-range dead-fire 2/4/6 launchers
   now use three-dice accuracy, individual missile packets, catalog ammunition
   hazards and native/Lua firing with rollback and saved recycle coverage.
   Standard/light/Magshot IS Gauss rifles now include inert ammunition, one-time
   weapon explosions, whole-mount destruction, CASE containment, crew effects
   and native/Lua replay; HGN-732 constructs unchanged. Heavy Gauss whole-section
   mounts now include spatial range damage, glancing reduction and moving-shooter
   recoil checks/falls with native/Lua rollback and restart. Complete relocated
   light/XL/XXL engines now retain type, mass, local critical losses and restart
   behavior; unchanged CES-4S fires through native/Lua interfaces. Explicit AC/20,
   LB/20-X and Heavy Gauss split links now resolve into one mount, including extension
   damage, primary-section explosions, section mass loss and native/Lua recycle replay.
   IS LB-X 2/5/10/20 now support slug and cluster ammunition, separate bin
   selection, cluster accuracy and individual pellet damage, with native/Lua mode
   controls, rollback and saved recycle replay. UM-R63 constructs unchanged.
   IS RL-10/15/20 and OneShot missile mounts now include independent persistent
   salvos, spent-state inspection, cluster damage, native/Lua rollback and restart.
   Failed Streak locks preserve the salvo; COM-4H constructs unchanged.
   Light AC/2, Light AC/5 and IS HeavyMachineGun now support ordinary ballistic
   fire, matching ammunition, critical damage and native/Lua persisted replay.
   IS Ultra and conventional/light autocannon two-round modes now cover supply
   fallback, hit grouping and loader failures. IS rotary burst selection, firing and
   recovery are covered below, together with supported IS gatling machine-gun fire.
   Installed targeting computers now assist eligible direct fire, with pulse
   stacking, LB-X cluster exclusion, critical/flood disablement, slot mass and
   native/Lua restart coverage. BL12-KNT constructs unchanged.
   Hardened gyros now include protected first hits, shared effective damage,
   mass, ground/airborne checks, cockpit feedback and persisted replay.
   XL and compact gyros now validate six/two-slot installations, scale mass,
   retain ordinary damage thresholds and preserve family through restoration.
   Small cockpit flags now adjust mass and shared piloting/standing targets,
   with a separate construction penalty and saved dice replay.
   Explicit Halfton bins now preserve capacity, ammunition type, mass and
   hazards through native/Lua firing and restart; OSR-3D and RZK-9S construct
   unchanged. Construction now infers bin size and normalizes initial quantities
   once; saved live ammunition is never refilled.
   Remaining BattleMech weapons,
   modes, physical attacks, sensors/electronics and configuration variants.
5. **Other unit classes — not delivered.** Ground-vehicle asset decoding is available
   (see the vehicle foundation section); live ground vehicles, VTOLs, naval units,
   aerospace/dropships, infantry/battle armor, turrets, transport and environment.
6. **Persistent support systems — not delivered.** Repairs, construction, salvage,
   economy, character progression and durable jobs.
7. **Autopilots and final coverage — not delivered.** Navigation/orders, radio,
   automatic targeting/combat, remaining scripting and operator interfaces.

Every gameplay slice must include command/Lua interfaces, storage, lifecycle,
documentation and tests. Unknown equipment, rule variants and unit behaviors must
not be accepted as operational. Interface changes should update the copied scripts
and docs directly, without compatibility shims.

## Reference evidence and test seeds

| Contract | Read-only reference | Rust evidence |
| --- | --- | --- |
| Map dimensions and conditions | `src/btech/map/map_conditions.c`, `map.c`; `tests/fixtures/unit/map_load/environment.map` | Non-square dimensions, environmental fixture, rejection tests |
| Terrain symbols/depth | `src/btech/map/map_terrain.h`, `map_terrain.c` | Typed terrain and surface-height tests |
| Critical ranges and templates | `src/btech/unit/template_load.c`, `template_loader.c`; `tests/fixtures/game/mechs` | JR7-D/AS7-D fields, slot ranges, modes and brand assertions |
| Equipment and recycle facts | `src/btech/unit/weapons_catalogue.c`, `weapons_vrt.h` | Typed initial catalog, JR7-D/AS7-D mount and ammunition grouping |
| Saved terrain encoding gap | `src/btech/map/map_coding.c`, `persistence/map_restore.c` | Opaque code preservation, explicit reload and versioned dictionary tests |
| Map geometry | `src/btech/unit/mech_geometry.c`, `equipment_types.h` | Center anchors, six neighbor bearings, signed vertical range |
| Startup timing | `src/btech/movement/mech_startup.c`, `core/btconfig.h` | Six five-second stages, operator override, abort, durable retry tests |
| Ground motion | `src/btech/movement/mech_update_speed.c`, `mech_update_motion.c`, `mech_motion_integration.c` | Acceleration, turning modes, terrain speed, continuous position restart, map-edge stops and live server controls |
| Conventional heat | `src/btech/movement/mech_update_heat.c`, `mech_update_speed.c`, `mech_overheat_modifier.c` | Fractional cooling, engine/sink damage, persisted heat, shutdown cooling, speed bands, reverse throttle, water/fire/temperature and recovery tests; overheat hazards pending |
| Context/heartbeat coupling | `src/btech/include/btech/context.h`, `core/heartbeat.c`, `core/btech_event.c` | Architecture constraint; runtime not yet implemented |
| Lua surface outside btech tree | `src/mux/lua/packages/btech` | Inspection and map mutation subset; no full callable parity claim |
| Combat and movement | `tests/unit/btech/combat`, `tests/unit/btech/movement`, `tests/unit/btech/sensors` | Pending rule characterization |
| Repair/autopilot integration | `tests/unit/btech/systems`, `tests/unit/btech/autopilot`, `tests/integration/autopilot_*` | Pending scenarios |

Future reference builds and scenario execution must use a copied checkout/build
directory under `stompymux-rs`, isolated game data, and no writes to `btmux-khi`.
Normal Rust builds/tests must continue to work without that reference copy.

Standing ground terrain LOS reports are implemented, including elevation, woods,
obscurants, water and partial cover. Sensor visibility and target acquisition
remain incomplete; see the terrain LOS contract in `btech.md`.

Unit target selection now supports current acquired optical contacts, eight-second
settling, durable restart, lifecycle cancellation and transactional notifications.
Hex targets, aimed sections, targeting computers and firing orchestration remain
pending; see the unit-target contract in `btech.md`.

## Reproducible construction audit

Run `cargo run --example btech_template_audit -- <mech-directory>` to emit JSON
with per-file parse/construction results, first rejection reasons and ammunition
normalization previews. The audit uses bounded asset reads and the same checker
as `@btech template-check` and Lua. It does not create objects or modify assets.

On 2026-09-08, the current `btmux-khi/game/mechs` directory contained 1,745 regular
files; 363 passed current biped construction checks. Leading first blockers were
276 unsupported Left_Side sections, 165 unsupported chassis-special sets,
121 unsupported Nose sections, 85 unsupported ammunition modes, 78 duplicate
specials fields and 45 IS.Anti-MissileSystem weapons. These counts identify the
first failure per asset and cannot establish that resolving it will make the
asset constructible. Construction success also does not prove complete gameplay
parity. Re-run the audit as rules and template support change.

A follow-up audit after additive repeated `Specials` parsing still reports
363/1,745 constructible files with no regressions. The 78 former duplicate-specials
parse failures now reach later checks and expose other missing capabilities;
removing a first blocker did not by itself make those assets constructible.

### Ammunition diagnostic follow-up

Construction normalization now attaches the section, one-based critical slot and
ammunition equipment name to errors. Unsupported-mode errors also include the
actual flags and weapon. This applies to the shared operator/Lua checker and the
maintained asset audit, including errors raised before loadout resolution.

The 2026-09-08 follow-up audit still constructs 363 of 1,745 regular files. Its
85 first ammunition-mode failures divide into 63 `Artemis/Mine`, 15 `Hotload`
and seven `Narc/Smoke` failures. These are first-blocker counts, not a promise
that implementing each flag will make that many files constructible.

Artemis is the largest next ammunition work item. Its end-to-end scope includes
controller-to-launcher links (including head-to-center-torso lookup), compatible
bins, selected ammunition, mode controls, the missile-table bonus, loss of the
controller and its feedback, and eventual ECM suppression. Reference entry
points are `unit/mech_ammunition.c::find_artemis_for_weapon`,
`combat/mech_weapon_modes.c`, `combat/mech_combat_missile.c` and
`combat/crit_mechs.c`. The lookup and critical handler use different link-index
expressions; characterize template loading and observable behavior before
choosing the Rust representation. None of these ammunition modes is enabled
by this diagnostic change.

First-slot mode declarations are now accepted for multi-slot mounts, with
empty or identical continuation modes and strict identity/data/brand validation.
This matches the shape of ARC-5R's launcher declarations without yet enabling
Artemis ammunition. Supported LB-X, one-shot and targeting-computer modes have
construction and restoration tests. The follow-up asset audit remains at 363
constructible files, with no newly constructible files or regressions.

Artemis controllers now have typed construction, one-ton mass, one-based
launcher-link inspection (including head-to-center links), critical/flood
availability and serialized restoration. Unassigned/dangling links are retained
without assistance. Lua unit state exposes detached controller records. The
Artemis ammunition/mode/cluster-bonus work item remains open.

The controller audit constructs 364 of 1,745 regular files: HSN-9F is newly
constructible, with no regressions. This is construction coverage, not evidence
that Artemis-guided ammunition is implemented.

Artemis-compatible ammunition, native/Lua selection, +2 missile-table effects,
glancing composition and raw-slot controller-critical mode clearing are now
implemented. ARC-5R constructs unchanged; native/Lua firing and callback rollback
share the same expenditure and saved state. ECM/Angel suppression remains part
of the outstanding electronic-warfare work.

The Artemis ammunition audit constructs 398 of 1,745 regular files, up by 34
from the controller-only audit, with no regressions. Newly constructible assets
include ARC-4M/5R/6S/8M, APL-1M/2S/3T, CN9-D, COM-5S/7S and ZEU-9T.
These counts measure current construction support, not complete combat parity.

Bin `Hotload` metadata is now accepted and retained separately from ammunition
selection and launcher fire modes. It does not alter supply, mass, readiness
or explosive contents, matching the reference's weapon-slot hotload lookup.
Launcher hotloading and jam handling remain open.

The bin-flag audit constructs 409 of 1,745 regular files, up by 11 with no
regressions: BSW-X1, CLN-7V, EXT-4A, HER-2M, LCT-3V, MHL-2L, MHL-X1,
RTX1-O, SDR-9KB, VTR-9A and VTR-9A1. This is construction coverage; it does
not claim that launcher hotloading is implemented.

Ammunition-feed jams now have persistent mount state, readiness/display support,
firing and mode-change guards, and explicit Rust record/clear primitives.
Invalid saved references are rejected. A persistence/control test verifies that
recycling does not clear a jam and rejected actions spend nothing. Hotload
triggering and player unjamming remain open; no template fire flag is enabled by
this groundwork.

Timed feed recovery now has native/Lua controls, one-attempt restrictions,
committed countdowns, piloting resolution, discarded ammunition and expiry
cancellation. Tests replay final-second outcomes through persistence. Hotload
firing, RAC recovery, XP and observer shell-ejection feedback remain open.

IS LRM/ELRM/LR-DFM hotloading now has template/native/Lua modes, configurable
minimum-range relief, lowest-two-of-three cluster dice, low-roll feed jams with
no expenditure, recovery and ordinary-supply critical explosions. Native/Lua
parity, persistence and tactical explosion replay tests cover these paths.
Clan launchers, general observer feedback and XP remain open.

Recovery failure-path tests now cover deletion-pending units, atomic multi-unit
rollback for malformed timers, and destroyed-weapon cancellation with unchanged
supply/dice after persistence. The recovery iterator now skips unavailable
objects consistently with the other simulation timers.

IS Ultra AC/2/5/10/20 construction and ordinary single-shot firing are now
implemented, including matching supply, mass, critical damage, recycle and
native/Lua restart tests. Ultra AC/20 split mounts are covered; double-shot mode
and its failure behavior are described below.

The follow-up audit constructs 425 of 1,745 regular files, up by 16 with no
regressions. Newly constructible assets include CTF-3D, DRG-5N, ENF-6M, SHD-5M,
STN-3L/3M and VTR-10D. This measures construction support, not double-shot parity.

Firing and unjamming now share a read-only ammunition draw planner with tested
cross-bin priority, shortages, damage filtering and ammunition-mode isolation.
The selector supplies both single-round firing/recovery and Ultra double-shot
expenditure, including shortage fallback.


Ultra double-shot firing now has template/native/Lua controls, two-round
cross-bin expenditure, doubled heat, two-shell hit grouping and glancing table
adjustment. Single-round supply persists normal-mode fallback; a roll of two in
active Ultra mode destroys the complete mount without expenditure. Integration
tests cover all four catalog weapons, misses, rollback and restart replay.
RAC, other rapid-fire families, XP and general observer feedback remain open.

The Ultra-mode audit still constructs 425 of 1,745 assets, with no construction
regressions. This step adds firing behavior to supported units; it does not
expand the supported chassis or equipment families.


Conventional IS and light autocannon rapid fire now uses the shared two-round
supply/heat/cluster path. Native/Lua controls, template mode, recoverable jams
on rolls three/four, catastrophic misload on two, and single-round fallback
have integration coverage. Misload damage bypasses armor and confines its
initial packet to the mounting section; nested explosions use existing rules.
Observer broadcasts, XP, rotary autocannons and gatling modes remain open.

The rapid-fire asset audit retains 425 constructible files out of 1,745, with
no regressions. This adds live firing behavior to already supported equipment.


IS Rotary AC/2 and AC/5 now support construction and ordinary single-shot
firing through the shared ballistic path, including catalog snapshot and
native/Lua rollback/recycle/restart coverage. Rotary burst flags, player burst
selection and rotary-specific recovery remain open; construction coverage
must not be read as full rotary combat parity.

The rotary catalog audit constructs 433 of 1,745 files, up by eight with no
regressions: GRM-01C, KW1-LH8, MDG-2A, SHD-5D, STN-4D, TLR1-OB,
TLR1-OTancred and UM-R70. This measures construction support only.


Rotary feed recovery now uses gunnery plus three, including extended-skill
configuration, disconnected-pilot fallback and a real roll while prone.
Boundary/restart tests verify both weapons, one-round successful expenditure,
failure preservation and exact dice consumption without XP mutation. Rotary
burst controls/firing and detailed pilot/observer feedback remain open.


IS rotary burst modes now include template/native/Lua selection, repeated-rate
idempotence, 2/4/6-round supply and heat, SRM shell-count tables, glancing,
rate-specific feed jams and single-shot shortage fallback. Tests cover both
weapons, all rates, cross-bin expenditure, misses, empty supply, rollback and
restart. Clan rotary equipment, additional observer feedback and XP remain open.

The rotary burst audit retains 433 constructible files out of 1,745, with no
regressions. This step completes the supported IS rotary burst firing path;
construction totals do not establish wider gameplay parity.


IS MachineGun/HeavyMachineGun gatling fire now includes template/native/Lua
controls, one prepared die for damage and heat, supply-limited triple-round
expenditure, glancing and atomic low/empty-supply handling. Tests verify exact
dice order, misses, cross-bin draws, glancing expenditure and saved replay.
Clan machine guns, additional observer feedback and XP remain open.


Precision ammunition now supports conventional and light IS autocannons:
template/native/Lua selection, half-capacity bins without double halving,
movement-modifier reduction, separate supply and rapid-fire composition.
Capacity/mass and persisted native/Lua shot tests cover all six weapons.
Other specialized autocannon ammunition and wider equipment remain open.


Flechette ammunition now supports the six conventional/light IS autocannons
against supported armored bipeds. Ordinary bin capacity, half-ton sizing,
separate supply, halved shell damage, glancing order and rapid-fire composition
have tests, including native/Lua saved replay shared with Precision cases.
Infantry/battle-armor target effects and other special ammunition remain open.


Armor-piercing ammunition now supports the six conventional/light IS autocannons:
AP templates, native/Lua selection, half-capacity supply, aim penalty and per-hit
front/rear armor critical checks compose with the shared rapid-fire resolver.
Boundary tests distinguish exactly half armor, below half, exact depletion and
penetration, including critical multiplicities and dice use. Remaining special
ammunition, other unit classes and wider combat parity remain open.


Caseless rounds now support conventional/light IS autocannons, including bin
sizing, native/Lua controls, independent rapid fire, ordered feed/ignition rolls,
internal ignition damage and surviving-bin expenditure. Tests cover normal and
short supply, callback rollback, saved failure replay and jam recovery. Wider
observer feedback, remaining special ammunition and other equipment remain open.


Incendiary rounds now support the six conventional/light IS autocannons against
armored bipeds, with native/Lua selection, template/live supply, rapid fire and
saved replay. Recycling-weapon critical ignition checks live matching supply and
uses the shared explosion cascade. Infantry damage and observer broadcasts remain
open; ordinary BattleMech hit damage does not establish those capabilities.


Direct-fire catastrophic misloads now stage observer broadcasts through the
shared native/Lua firing action. Audience snapshots use current acquired optical
contacts before damage and exclude the subject, unavailable/offline units and
other maps. Tests cover read-only filtering, ignition versus surviving jams and
callback rollback. Critical/thermal explosions and other observer events remain
open; this does not complete general battlefield notification parity.


The shared tactical explosion cascade now emits observer notices for ammunition,
Gauss, hotloaded-launcher and incendiary explosions, including the thermal caller.
Owned notice text and catalog formatting replace per-weapon message duplication.
Observed criticals and destructive ammunition explosions have tests, with restart
replay and read-only audience filtering. Other battlefield broadcasts and full
sensor/identity parity remain open.


Unjamming expiry now carries typed private pilot roll details and cockpit outcome
messages, with success-only observer shell-ejection feedback. Tests cover rotary
threshold/restart reporting and pilot/cockpit/observer routing, including prone,
empty-supply, failure and absent-pilot cases. XP and other skill-system work remain
open; this completes the currently supported unjamming feedback path only.


Ordinary launched direct shots now include third-party firing feedback based on
pre-damage visibility of shooter and target. Tests cover hit/miss wording, hidden
identities, powered-off observers, native/Lua output, callback rollback and saved
replay. Participant cockpit feedback remains separate; failed launch paths retain
their own notices. Hex-target broadcasts and full sensor/identity parity remain
open.


Startup completion and moving-ground-shutdown observer feedback now flow through
the shared transactional notice path. Tests verify final-stage timing, no repeated
or cancelled-startup broadcasts, restart replay, the strict forward-speed boundary
and native/Lua shutdown rollback. Broader startup hazards and other unit classes
remain open.


Successful stand-timer completion now emits observer feedback; failed-attempt
recovery does not. Tests cover countdown/restart behavior, completion after
shutdown, unchanged dice and no duplicate messages. Other movement and physical
combat feedback remains open.

Free-fall landing observer feedback now uses pre-impact contact visibility within
the airborne transaction. Light/destructive impacts, shut-down observers, rollback,
restart replay and no-duplicate completion have regression coverage. Other movement
observer events and the broader acceptance gates above remain open.

Supported jump launch and landing paths now emit shared observer notices through
native/Lua commands and automatic ticks. Coverage includes early-landing success
and failure, missing-leg/actuator/gyro failures, saved replay, callback rollback
and suppression of graceful landing during crowding collisions or avoidance.
DFA, jump stagger checks and other movement observer events remain open.

Stand attempts now emit attempt/failure observer feedback, captured with the
outcome and damage in a single domain result. Native/Lua order and rollback,
normal/anyway/careful success/failure, observer shutdown and saved replay are
covered. The broader rule, interface and paired-characterization gates remain open.

Crowding observer feedback now covers ground bumps, jump/fall landings and near
collisions, with independently hidden participant identities and pre-damage
snapshots. Non-ground avoidance failures include the configured fall broadcast.
Domain replay/rollback and native/Lua landing output are covered. The shared
participant visibility resolver also serves existing firing broadcasts.

Thermal-shutdown observer feedback now covers grounded stops, airborne falls and
failed moving-ground balance. Tests cover stationary/forward/reverse boundaries,
successful overrides, powered-off observers, repeated checks and saved replay.
Other thermal equipment effects, unit classes and broader acceptance gates remain open.

Lua torso rotation and arm flipping now use the same Rust controls and cockpit
messages as native commands. Native/Lua state and output, case-insensitive direction
aliases, limits, rejected controls, caught/propagated callback failures and saved
replay are covered. Neither control broadcasts to observers in the reference.

Native/Lua speed and heading feedback now uses shared domain notices addressed to
the cockpit, replacing caller-only native confirmation and silent Lua mutation.
Recipient routing, normalized headings, named/numeric speeds, saved replay and
caught/propagated callback failures are covered; movement rules remain shared.

Shared ice/bridge fractures now capture breaker and occupant observer messages
before terrain replacement. Downward/upward ice feedback, neighbor ordering,
submerged-breaker concealment, zero-depth ice, bridge height selection, rollback
and saved replay have coverage. Terrain-targeted weapon blast feedback and
remaining environment triggers remain open.

Traditional and rolling stagger checks now stage observer feedback with the
resolved report. Retain/consume severity levels and their failed-fall wording are
covered separately from traditional failure-only broadcasts, including saved
cadence replay and observer shutdown. Broader casualty/XP and movement gates remain open.

Immediate critical/section-loss balance feedback now reaches observers through the
shared tactical damage transaction. Ground cause distinctions, successful checks,
observer shutdown, gyro replay and last-jet/deferred-actuator behavior are covered.
Other component-damage feedback and the broader combat/character gates remain open.

Engine-critical observer smoke now uses pre-loss power and visibility, including
the fatal engine hit. Three hit stages, stopped engines, observer shutdown,
saved replay and repeated critical loss have coverage. Other component damage
feedback and the broader equipment/character gates remain open.

Hip/leg-actuator observer feedback now shares pre-critical component visibility
with engine smoke. Coverage includes limb distinctions, hip-loss suppression,
shutdown/visibility guards, repeated slots, deferred airborne balance and saved
replay. Other component feedback and the wider equipment/physical-combat gates remain open.

Limb-critical cockpit feedback now covers shoulders, hips, upper/lower arms,
hands and leg actuators, including shutdown and prior hip loss. The expanded
limb matrix checks both sides, observer/cockpit ordering and saved replay.
Remaining component messages and broader combat gates are still open.

Core-critical cockpit messages cover engine, sensor, life-support and ordinary
gyro damage stages, alongside existing hardened gyro feedback. A seeded matrix
checks powered/stopped states, repeated slots and saved replay through fatal
engine loss. Attacker-specific destruction feedback and remaining
component/casualty behavior still require coverage.

Gyro observer buckling effects now cover ordinary and hardened first-hit stages,
including the hardened shutdown exception, pre-loss visibility, repeated slots
and saved replay. Airborne failures retain screech-before-fall ordering.
Attacker-specific destruction feedback and remaining component/casualty gates
remain open.

Cockpit-critical occupant/observer feedback now accompanies durable unit
destruction and pilot release, with pre-loss visibility even while shut down.
Coverage checks saved replay, repeated slots and in-character casualty rollback.
Attacker feedback and the actual in-character casualty lifecycle remain open.

Conventional weapon destruction/non-working-mount feedback and heat-sink
occupant/green-mist effects now have power, visibility and saved-replay coverage.
Grouped double sinks and repeated unavailable slots are checked explicitly.
Enhanced partial weapon damage and remaining equipment/casualty gates remain open.

Jump-jet plasma observer feedback now shares the component-critical snapshot.
Ground power/visibility/repeat coverage and saved airborne replay check both
continued flight and flare-before-last-jet-fall ordering. Remaining
equipment/casualty gates are still open.

ImprovedJJ_Tech now supports complete same-section two-slot jet installations
with one MP per group, doubled jet mass, grouped loss/flooding and section loss.
Native/Lua launch, rollback, heat, continued flight, last-jet falls and saved
replay are covered for either slot in every group. Invalid pairs, inconsistent
capacity and partial persisted pair losses are rejected. This adds equipment
coverage; mechanical jets and the broader construction/unit-class gates remain open.

Heavy flamers now support fuel, ballistic skill, normal/heat modes, catalog
range/heat/mass/recycle and conventional ammunition hazards through shared Rust
firing. Native/Lua, rollback, restart, empty/last-round supply and heat-mode
hit/glance/miss boundaries are covered. The current asset audit remains at
433 constructible files out of 1,745; broad equipment and unit-class gates remain
open. MechanicalJJ_Tech is explicitly unimplemented in the reference and remains
unsupported rather than enabling an invented movement rule.

Coolant guns now use shared firing/expenditure with non-damaging external or
self cooling, normal/heat controls, explicit cooling reports and contact-free
self application. Tests cover fuel, hit boundaries, negative heat credit through
the next sample, native/Lua routing, rollback and saved replay. Remaining weapon
variants, terrain interactions and broader equipment/unit-class gates remain open.


Vehicle flamer variants now share the fueled-flamer mode and saved-replay matrix,
including normal/heat fire, empty/last/full fuel, native/Lua parity, callback
rollback and hit/glance/miss heat boundaries. Catalog snapshots check their
distinct mass, range, fuel and recycle facts. This extends weapon coverage;
vehicle chassis and the broader equipment gates remain open.


Acid throwers now use the shared ballistic construction, damage and firing path.
Catalog/range checks, complete two-slot mounts, unsupported heat-mode rejection,
per-slot weapon loss, ammunition mismatch/explosions, native/Lua rollback and
saved heat/recycle replay are covered. Remaining weapon families and broader
equipment/unit-class gates are still open.


IS plasma rifles now carry their post-damage heat effect through the shared
impact path, with inert ammunition and public per-impact heat rolls. Target
dice and saved replay cover isolated hits, multi-section transfers and fatal
core overflow; native/Lua firing shares the conventional fire/recycle matrix.
Other unit classes and remaining equipment/combat gates are still open.


All four IS Thunderbolt launchers now use shared missile firing, cluster rolls,
hotloading and ammunition hazards. Catalog snapshots, every cluster result,
glancing payloads, supply boundaries, individual mount criticals and native/Lua
saved fire/recycle replay are covered. AMS interception, indirect-fire commands
and the broader equipment/unit-class gates remain open.


Hyper autocannon variants now share conventional ballistic firing and saved
native/Lua replay, with construction, range, supply, per-slot loss and ammunition
hazard checks. Rapid-fire and RFAC-specific ammunition flags are rejected.

MML-3/5/7/9 implement functioning combat as requested, deliberately departing
from the reference's missing missile-hit tables. Shared launcher rules select
SRM or LRM profiles for range, cluster damage, supply, interception and ammunition
hazards across supported chassis. Native/Lua selection, firing, rollback and
restart are covered in `tests/btech_mml.rs`. Long-range special rounds are
described in the MML LRM special rounds section of `btech-delivery.md`.


Seventeen Clan energy identities now use shared construction and firing.
Catalog snapshots, complete/grouped mount losses, accuracy families,
targeting-computer eligibility, Clan flamer hit boundaries, native/Lua
fire/recycle replay and Clan-versus-IS plasma effects are covered. Clan chassis,
remaining Clan weapons, underwater firing and other broad gates remain open.


Clan Gauss and three machine-gun variants now share existing ballistic rules.
Catalog, ranges, per-slot mount loss, ammunition hazards and native/Lua
fire/recycle replay are covered. The gatling saved-replay matrix now includes
all three Clan guns across low supply and hit/miss scenarios. Clan chassis,
remaining ballistic/missile families and broader gates remain open.


Ten conventional Clan missile launchers now use shared firing and supply rules.
Every cluster-table result, Clan LRM glancing shifts, mount criticals,
ammunition explosions, normal/native/Lua saved firing and one-shot/Streak
hit/miss rollback/restart behavior are covered. Clan chassis, ATM, remaining
ballistics and broader combat/environment gates remain open.


Eight Clan LB-X/Ultra autocannons now share catalog, firing and ammunition
behavior. LB-X cluster distribution/mode/replay tests and Ultra two-bin,
fallback, loader-failure and saved native/Lua replay matrices include them.
Clan UltraAC/20 participates in every supported split-mount direction and
critical-loss test. Clan chassis, ATM, AMS and broader gates remain open.


Clan biped construction now includes implicit double heat sinks, paired
critical loss and cooling/mass, built-in CASE, complete standard/compact/XL/XXL
engine layouts and Clan material thresholds. Tests cover grouped-loss
validation, engine side loss, material boundaries, containment/restart and a
native/Lua Clan firing scenario. Unchanged MadCat-A, Vulture-C and Vixen-1 assets
provide combined construction coverage. The asset audit rises from 440 to 709
of 1,745 constructible files. Remaining chassis equipment, sensors, physical
combat, other unit classes and later acceptance gates remain open.

### Searchlight hardware and optical illumination

Native Rust searchlights support the five-second `slite` transition, assigned-pilot checks, Lua transaction rollback, saved countdowns, front-torso damage rolls and observer notices. Illumination is derived from live geometry for optical contact evaluation rather than maintained in a duplicate target-state cache. The reference's unpowered switch expiry leaves the current setting unchanged. Unit illumination warning preferences and their transition messages are supported through `mechprefs SLWarn` and Lua. A saved notification observation suppresses repeat warnings and never controls optical visibility.

The template construction audit accepts **770 of 1,745 assets** with searchlight hardware enabled, up from 709. Construction acceptance does not imply complete gameplay parity.

### Clan rotary autocannons

The four Clan rotary families use their catalogue mass, range, heat, critical-slot, ammunition and recycle values through the shared Rust rotary implementation. Coverage extends shell-group/glancing tables, normal direct fire, two/four/six-round bursts, ammunition fallback, jam thresholds, gunnery recovery, native/Lua transactions and persisted replay. Burst and recovery scenarios use Clan chassis. No parallel Clan firing implementation or FFI layer is introduced.

### Armor and ammunition warnings

Enabled-by-default armor and ammunition warning preferences are available through native `mechprefs` and Lua. Tactical armor phases report integer-threshold severity transitions after through-armor critical resolution. Shot reports carry pre-expenditure ammunition warnings, with single/double/gatling windows and the reference’s mode-dependent bin weighting. Warning decisions consume no dice; delivery uses the existing combat transaction. Ammunition dumping remains outside this milestone.

### Friendly-fire safety

Direct shots honor the persisted unit `FFSafety` preference and map flag 256 before dice, heat or ammunition expenditure. Team identity is shared with sensor/scenario data. Coolant fire remains exempt for both external and self-directed shots. Native/Lua rejection, rollback, enemy fire, preference toggling and restart are covered. Physical attacks will need the same policy when their command paths are implemented. The preference command now uses one metadata table for all supported settings.

### Cockpit status projection

Native `status` and Lua `unit.status` share a read-only renderer for supported biped state, with armor/info/weapons/heat selectors and a short view. Weapon formatting is shared with `weapons`; damaged, unplaced and shut-down units remain inspectable. Native/Lua parity, unchanged world state, damage and persisted replay are tested. ASCII armor diagrams, alternate R layouts, remaining status annotations and other unit classes remain open; this is not full status-command parity.

### Compact status and condition detail

Status N/NW follows the compact chassis record and independent weapon/ammunition columns, including mode letters, zero-ammunition pruning and surplus ammunition rows. The reference computes armor export fields into a different buffer and does not emit them; NA consequently matches N. Info views expose live jump/fall state, mechanical speed/jump limits, sensors, target settling, standing recovery, landing stabilization and unjam countdowns. Output is derived without advancing state and survives restart.


Biped kicks now have a native Rust resolver shared by native commands and Lua: targeting and actuator guards, physical hit tables, glancing damage, impact cascades, balance/falls, and durable limb recovery. Native/Lua parity, rollback, deterministic roll boundaries and saved timer expiry are tested. Punches, clubs, charge/DFA, TSM and physical-combat XP remain open; this does not close the physical-combat gate.


Biped punches now share physical targeting, hit resolution and durable recovery with kicks. Native/Lua one- and two-arm actions preserve left-to-right sequencing, continue past expected per-arm rejections, and roll back complete damage cascades on errors. Tests characterize arm/torso arcs, elevation tables, missing actuators, miss/glancing rolls without kick balance checks, native/Lua messages and restart replay. Clubs, physical weapons, charge/DFA, TSM, other classes and physical-combat XP remain open.


Biped trips now use the shared Rust physical resolver and native/Lua adapters. Tests cover kick-like reach and recovery, absent actuator aim penalties, target posture/rising guards, no direct damage or hit-location dice, target balance/fall outcomes, missed-trip dice, glancing modes, rollback and restart. Other physical weapons, charge/DFA, quad rules and physical-combat XP remain open.


Cockpit-status rendering now retains the styled report contract through native delivery, including surplus-ammunition colors. Literal identity fields and weapon annotations are escaped. Rendering tests exercise color-disabled and ANSI clients, color thresholds, bracket-containing unit names, `[OS]` labels, native/Lua source parity and read-only state. Remaining status layout and annotation gates are unchanged.


Triple Strength Myomer is supported for constructed bipeds: complete passive installations, zero mass, noncritical slots, sampled-heat activation, physical base-damage doubling, and distinct throttle/turning/update/aim calculations. Tests cover native/Lua controls, heat boundaries, passive loss, damage rounding and cooling/restart motion. Cargo/towing, sprint/booster combinations and other classes remain unfinished; this does not close the equipment or movement gates.

The TSM asset audit constructs 776 of 1,745 regular mech files (six more than the preceding 770-file baseline). This checks construction support, not complete gameplay parity.


Axes and swords now have native Rust construction/mass and critical-slot behavior plus shared native/Lua physical actions. Tests cover surviving-part thresholds, hand/actuator guards, damage and TSM/glancing rounding, hit tables, per-arm selection, recovery, rollback and restart. Other physical weapons, charge/DFA, other classes and physical-combat XP remain open.

The axe/sword asset audit constructs 787 of 1,745 regular mech files, up from 776 after TSM. The eleven additional constructions do not imply full gameplay parity for those designs.


Maces now share native Rust construction, mass, critical loss and arm-weapon actions. Coverage includes the operational-part threshold, actuator/TSM/glancing rules, missed-swing piloting at +2 with success/fall outcomes, native/Lua transactions and saved recovery. Clubs, claws, saws, charge/DFA, other classes and physical-combat XP remain unfinished.

The mace asset audit constructs 787 of 1,745 regular mech files. Construction acceptance does not establish full gameplay parity.


Dual saws now support native Rust construction, critical loss and native/Lua arm attacks. Tests verify fixed seven-point base damage, no TSM boost, seven operational parts, missing-hand operation, +1 aim, the reference's fixed left-arm location without location dice, glancing damage and saved transactional recovery. Claws, clubs, charge/DFA, other classes and physical-combat XP remain unfinished.

The saw construction audit accepts 787 of 1,745 regular mech assets. This is construction coverage, not complete gameplay parity.


Claws now support native Rust construction, surviving-part availability and two-arm native/Lua physical actions. Coverage includes tonnage/TSM/glancing damage, fixed left-arm damage and dice, actuator behavior, mass, default arm selection, rollback and saved recovery. Clubs, charge/DFA, other classes and physical-combat XP remain unfinished.

The claw construction audit accepts 787 of 1,745 regular mech assets. This checks construction support rather than full gameplay parity.


Tree acquisition and two-handed clubs now use owned Rust state and the shared physical transaction. Native/Lua controls cover carrying, explicit release, forest-supplied attacks, hit-only breakage, punch restrictions, shutdown/hand-loss drops and durable arm recovery. Tests cover mechanics, transactional parity and persisted carrying state. Charge/DFA, physical XP, pod-removal interactions and other unit classes remain unfinished.

One-way biped charge collision resolution now has read-only profiles and an atomic Rust resolver. Tests cover relative-speed/current-mass formulas, distance versus velocity policy, recoil settings, rejection without mutation, hit/miss packets and balance, and saved six-section recovery. Charge commands, movement tracking/timeout, mutual charges and end-to-end parity remain pending, together with DFA and the broader unfinished gates.


One-way charge recoil now respects damage-phase ordering: target damage and immediate falls precede recoil velocity/direction sampling. The report separates predicted recoil from actual applied recoil. Seeded moving-target leg loss verifies the distinction; misses retain zero recoil. This fixes a prerequisite for mutual-charge reuse and does not close the pending charge movement or mutual-charge gates.


### Mutual-charge characterization

Inspection of charge_mutual_resolve in combat/mech_charge.c shows that two calls to the one-way resolver would not preserve behavior:

- Eligibility is evaluated independently before either attack. Mutual ranged recovery checks the six charge sections rather than native indices zero through five. Unpowered, unconscious, blinded and stunned attackers are rejected. Jumping disables both attempts; a prone opponent disables the corresponding incoming attack.
- The first participant's front arc ignores torso rotation; the second uses its weapon arc. The reference then merges the second unit's torso bits into the first before deciding whether either can proceed. The explicit merged torso state and its rightward geometry are characterized below.
- First-participant damage uses relative speed without the one-way final +1. Second-participant damage uses its own (actual tons + 5) / 10, plus specialization.
- If either participant remains eligible, both attack rolls are drawn before damage. The second eligibility and roll survive the first collision. If neither is eligible, no attack rolls or recovery are applied.
- Each successful attack completes damage, recoil, stopping and two piloting checks before the next attack. First-participant level-three recoil uses its own mass; second-participant recoil uses its opponent's mass. The second recoil loop uses calculated recoil for full five-point groups but its inflicted-damage remainder for the final packet.
- After any accepted mutual attempt, both units receive six recovery timers, including an ineligible participant. The movement owner must clear both selections without dispatching another collision.

These findings define the mutual collision component implemented below. Movement dispatch, commands and full game-loop charge parity remain pending.


The native Rust mutual collision resolver now freezes both eligibility decisions and attack rolls before damage, preserves asymmetric damage/recoil and control order, and applies six-section recovery to both participants whenever either attempt is accepted. Tests cover hit/miss pairs, an ineligible participant, no eligible participants, torso merging, fixed second-attempt eligibility after first-impact leg loss, recoil remainder behavior and saved recovery. Native/Lua charge initiation, movement tracking, timeout and automatic collision dispatch remain unfinished; no end-to-end charge completion is claimed.


Ground charge initiation now uses native `charge [#unit|-]` and Lua `btech.unit.charge` with owned, persisted target/time/distance state. Ground movement integrates distance, expires new-rule selections on update 61, and dispatches one-way or mutual collisions once at the endpoint. Retargeting preserves counters, explicit cancellation clears them, and shutdown clears only selection. Tests cover native/Lua transactional parity, saved intent, paused stationary timers, turning without collision, distance accumulation, deterministic hit/miss dispatch, and mutual attack dice/recovery. Airborne charge tracking, complete charge eligibility (including unimplemented movement modes and blindness), DFA, physical XP and the remaining broader gates are still pending.


### Charge tracking during jumps

Ground and jump updates now share `BattleMovementRules`. Active jump updates age new-rule charge selections without adding travel distance; stabilization and free-fall events do not run the charge timer. Integrated airborne endpoints check collision range and reject airborne attacks without spending attack dice or applying recovery. Mutual airborne rejection clears both selections. Landing and terrain-entry consequences precede the endpoint check, so normal landing uses grounded eligibility. Bridge interruption and lost thrust retain their early exit before endpoint collision dispatch.

Focused tests verify timer expiration and persisted flight replay, old-rule counter preservation, unchanged charge distance, one-way/mutual airborne rejection without combat dice or damage, landing-before-eligibility order, and inactive tracking during stabilization. This closes the conventional jump-tracking gap noted above. Unimplemented charge eligibility modes, blindness, DFA, physical XP and other unit classes remain open.


### DFA landing calculation component

The read-only Rust `battle_dfa_profile` now characterizes biped landing eligibility and damage forecasts. The caller retains airborne state until after calculating aim. Eligibility requires the same map and occupied hex; it checks ranged recovery in arms, legs and side torsos, and physical recovery in arms/legs only. Airborne targets and map-prohibited friendly fire reject the attack. Fixed base aim is five; pilot-based aim uses the pilot target directly. Melee specialization changes attacker movement to `min(0, movement) - 1`; target motion includes immobility and close-range prone adjustments. Aim above twelve declines without mutation or dice.

Impact damage uses floor(3 * actual whole tons / 10), plus one when nominal tonnage has a remainder modulo ten, plus specialization. Recoil uses nominal tons / 5. Initial target hits use the punch table when standing and the weapon table when prone. The eventual resolver must resample posture for each packet, because earlier damage can cause a fall.

Verified coverage includes mass/nominal-rounding separation, airborne aim, specialization, immobile/prone targets, limb versus weapon recovery, moved targets, configured pilot aim, and rejection without changing state. DFA damage resolution, landing integration, targeting commands and transactional replay remain unfinished.

Reference follow-up: `mech_bth_movement.c::mech_target_movement_modifier` includes a -4 immobility adjustment. The DFA component and ordinary physical attacks account for represented immobility; the charge component currently uses only target speed and needs this correction with updated characterization tests. Blindness and fortification remain unrepresented eligibility states.


### Charge target immobility correction

Charge, DFA forecasts and ordinary grounded physical attacks now share the close-range target movement calculation. In addition to speed, shut-down or unconscious targets receive -4 aim; prone biped targets receive -2 where attack eligibility permits them. Charge still rejects prone targets before calculation. This corrects the missing charge adjustment identified during DFA characterization. Blindness and fortification remain unrepresented states.

Tests explicitly compare the same roll against operational and shut-down targets in both one-way and mutual collisions, and verify unconscious-pilot aim without changing damage or consuming profile dice. The existing miss/recovery and torso-merge fixtures now use operational targets so they continue to exercise actual misses. Tests for all existing physical attacks and DFA use the shared calculation without changing their rules.


### DFA damage resolution

`resolve_battle_dfa` now resolves an accepted biped landing attack in a single world candidate. It calculates aim before ending airborne state, draws the attack roll, and applies target damage followed by recoil in groups of at most five. Standing targets use punch locations; each later packet resamples posture so damage-triggered falls switch to weapon locations. Hit recoil uses front kick locations. If recoil leaves the attacker standing, attacker +4 and target +2 control checks follow in that order, each with ordinary fall consequences on failure.

Misses use rear weapon locations for nominal-tonnage recoil, then a +2 pilot-injury check and explicit prone state. They do not call the ordinary fall resolver merely to become prone, avoiding duplicate damage. Terrain elevation and flooding are applied, including sub-surface ice elevation. Both outcomes apply six recovery timers. Reports retain ordered impact cascades, checks, injuries and notices; a late failure discards the entire candidate.

The shared impact engine now distinguishes admission of a new attack from continuation of an accepted packet sequence. DFA and charge packets continue after an earlier packet destroys the unit, preserving subsequent location/damage processing. Starting a new direct attack on a destroyed unit still fails.

Tests cover hit/miss damage distributions, control ordering, injury, replay, saved recovery, target knockdown changing subsequent locations, continuation after a torso kill, late rollback and ice immersion. DFA targeting commands, durable jump intent, landing dispatch, complete landing interaction parity and physical XP remain unfinished. This component does not yet make targeted jumps available to players.


### Durable targeted-jump launch state

The Rust `launch_battle_dfa` API selects an explicit acquired target or the current target lock and fixes its destination hex at launch. Both forms require a visible live tactical target on the same battlefield and use normal jump authorization and route checks. Target range is sampled at launch for capacity and apex; the snapped destination center remains independent of that range. Saved targeted paths preserve these inputs and reject contradictory projection/target-range admission modes.

The flight cursor owns the optional DFA target identity, so progression and save/load retain it while shutdown, destruction or aborted flight remove it with the flight. Subsequent target motion does not steer the path. Flight validation rejects self-targeting but does not require a vanished target to remain present; landing must handle that case.

Tests verify launch-time range versus snapped distance, fixed destination under target movement, persisted flight replay, selection rejection without mutation, shutdown cleanup and stored-path validation. Lua state type annotations include the new fields. Targeted-jump landing dispatch and native/Lua command exposure remain pending: this launch API is a component for that integration, not a completed player-facing DFA action.


### DFA landing dispatch and commands

Native `jump #unit` and argument-free `jump` now select a DFA target; coordinate jumps retain their existing syntax. Lua uses `btech.unit.dfa(unit, pilot, target)`, with nil for the current lock. Command and Lua selection share the launch transaction and rollback staged output together. Status exposes the flight's DFA target.

Normal and pilot-requested early landings share `BattleMovementRules`, including configured physical pilot skill, movement modifiers, hit arcs and fall rules. Landing inspects DFA eligibility while the flight still supplies the airborne aim modifier. An accepted attack resolves once, clears flight intent, and suppresses ordinary completion/crowding messages and the twelve-second jump stabilization timer. Moved or ineligible targets fall back to normal landing. Subsequent damaged-gear checks and flooding still run. Completed flight cursors are permitted only inside a landing transaction; world validation prevents committing an unresolved arrival.

Tests cover fixed-base airborne aim at actual arrival, saved replay, one-shot dispatch, normal fallback without attack dice, native/Lua default and explicit selection with rollback, and early-landing hits under pilot-based rules. Existing movement, physical and landing suites remain regression gates. Broader landing interactions (including full stagger behavior), unsupported unit classes, special movement/eligibility states and physical XP still require further characterization; this does not claim complete BattleTech parity.


### Jump stagger and experience dependencies

Reference inspection distinguishes `rd.stagger_damage` from both rolling damage history and traditional turn damage. `mech_condition_summary.staggering` and jump/landing modifiers read that separate signed counter. Ordinary damage appends history or turn damage; the counter is reset to zero, set to -10 by the older event handler, or restored from a snapshot. Consequently the Rust rolling history must not be substituted as a trigger for the separate jump-specific check. Restored nonzero-counter semantics remain a distinct compatibility/characterization question.

Physical damage experience is restricted to in-character participants and active pilots in the reference, so tactical simulation attacks must not start awarding it. In-character casualty/action integration remains unfinished.

The shared Rust experience component now calculates awards under explicit catalog policy: strict thirty-second intervals, continuous-XP and explicit-override exemptions, low-24-bit balance, raw-skill-based thresholds scaled by factors of three, and strict `balance > threshold` earned-level boundaries. Recalculation retains total balance rather than spending it, and stored earned bonuses do not feed back into the raw target. Arithmetic is bounded for large inputs instead of relying on signed overflow. `award_battle_character_experience` updates named character values atomically; game-action eligibility and catalog lookup remain the caller's responsibility. Tests cover timing boundaries, wrapping, cumulative thresholds, zero-threshold behavior, extreme values, unchanged state on rejection, and saved/reloaded awards. Combat XP hooks are not enabled by this foundation.

### Canonical skill catalog

The Rust catalog now centralizes skill names, categories, default XP thresholds and continuous-award flags. Named awards resolve case-insensitive full names before generated aliases and store canonical keys. Alias collisions retain the first catalog entry, matching the reference lookup behavior. Lua exposes detached metadata through `btech.character.skills()`. Tests cover every catalog name and alias, collision priority, representative policies, canonical storage, rejected awards, persistence, and detached Lua results. Action-specific eligibility and combat XP hooks remain unfinished.

Runtime threshold administration now shares a wizard-authorized Rust setter across native commands and Lua. Named awards read effective thresholds; catalog inspection retains defaults. Thresholds reset on database reload, matching reference context initialization, while character XP persists. Tests cover authority, invalid values, alias lookup, zero-threshold recalculation, native/Lua access, callback rollback and reload behavior.

### Skill progression inspection

The Rust and Lua progression query now exposes the reference's cumulative next-level balance, with a separate remaining-points field. It shares raw-skill cost scaling with awards, includes the strict boundary, reads runtime thresholds, and retains stored bonuses until a later award recalculates them. Tests cover successive boundaries, threshold changes, disabled progression, unknown skills/characters, detached Lua results and bounded extreme arithmetic. This is read-only character inspection; combat XP eligibility and award hooks remain separate unfinished work.

### Proportional experience retention

Reference evacuation can retain a configured per-thousand fraction of character XP and recalculate earned levels. The shared Rust operation now performs this arithmetic under current skill thresholds while preserving base values and timestamps. It publishes all skill changes together, rejects invalid fractions and unknown XP-bearing names, and persists through the existing character-value storage. Tests cover half/full/zero retention, earned-level recalculation, unchanged advantages, atomic rejection and database reload. This prepares casualty integration; evacuation triggers, wizard exemptions, in-character damage and casualty movement remain unfinished. Rust uses bounded unsigned XP arithmetic instead of reproducing signed overflow behavior.

### Crew evacuation through movement

A wizard-controlled Lua evacuation path now reuses ordinary teleport callbacks and cockpit reconciliation, applying configured XP retention after successful moves. It preserves wizard occupants, ignores tactical units and snapshots occupants before callbacks. All moves, XP changes and staged effects roll back on failure. This is an intentional atomic operation rather than a partially completed multi-occupant transition. Integration tests cover authorization, wizard exemption, relocation, pilot release, XP loss, repeat calls, callback abort, a post-move XP validation failure and database reload. Automatic in-character damage/casualty triggers, ejection vehicles and their delayed disposal remain unfinished.

### Material casualty classification

Reference biped section destruction distinguishes ordinary CT wrecking from lethal head loss; cockpit criticals also invoke crew evacuation. `BattleImpactReport::crew_casualty()` now exposes head/cockpit loss from the actual cascade rather than inferring death from `destroyed`. Tests resolve real head, CT and cockpit damage, plus a nonlethal head hit. Head loss takes precedence when both occur in one cascade. Fatal character injury, flooding, reactor events, other unit classes and automatic publication remain separate casualty work; this query alone does not enable in-character combat.

### Atomic material-impact casualty action

The host-level impact action now connects material casualty classification to evacuation in one checkpoint. A failed destination or callback restores damage, dice, crew assignment, movement, XP and staged effects. Integration tests distinguish CT wrecking from head-loss evacuation, fail the destination after damage resolution and verify rollback, then save/reload a successful casualty. This completes publication for this material-impact action only; ordinary firing/physical paths still require their remaining character injury, environmental and eligibility work before in-character combat can be enabled.

### Character-mode pilot injury and fatal evacuation

In-character pilot injury now composes existing character health with consciousness/recovery and explicit unit casualty state. Its separate persisted status avoids treating the tactical six-hit limit as RPG death. A host action publishes fatal evacuation atomically. Tests cover zero damage without mutation, survival at six injury points, unconscious follow-up without extra dice/countdown reset, nonfatal persistence, failed fatal evacuation rollback and saved fatal loss. Unit status and Lua inspection expose character-mode counters. Firing/physical/environmental callers are not yet switched to this operation; their ordered injury and casualty publication remains integration work.

### Ordered character injuries in impact actions

The host impact action now resolves character head/explosion injuries inside the material cascade, using the existing player health and recovery routines. Reports retain ordered applied character injuries; handled effects are not returned for duplicate replay. Fatal health loss can trigger evacuation without head or cockpit destruction. Tests verify a fatal one-point head hit and surviving ammunition explosions with and without Pain Resistance, alongside existing callback/destination rollback checks. Normal weapon/physical command entry points remain gated while their full in-character damage, fall, environmental and XP paths are integrated.

### Character stun and private consciousness feedback

Character impact actions now consume crew stun through the shared tactical/character stun implementation. Tests cover zero-damage no-op, one notice per recipient, removal from pending effects, failed casualty rollback without leaked notices, and saved timer expiration. Ordered character injuries and direct character injury actions publish consciousness feedback to the pilot only; a second cockpit occupant test verifies that these messages stay private. These action improvements do not remove the remaining normal-combat gates.

### Cockpit flooding correction and casualty action

Reference `environment_damage.c::mech_parts_destroy` destroys a mech on a breached head regardless of IC mode; only occupant evacuation is IC-specific. Rust now derives destruction from the flooded head, while preserving intact structure and the distinction between disabled and destroyed criticals. The prior tactical expectation of a surviving flooded cockpit was corrected. The host flooding action publishes new cockpit evacuation atomically. Tests cover tactical destruction, unchanged structure, character evacuation, wizard exemption, failed destination rollback without leaked notices, repeat calls and saved destruction. In-character support-loss falls remain an explicit downstream gap.

Ordinary movement/damage flooding entry points retain their character-mode rejection until they can publish casualties. Only the explicit host flooding action uses character-enabled immersion. The water-entry rollback regression exposed and now protects this boundary, preventing an ordinary heartbeat from committing a character cockpit death without evacuation.

### Character dry-ground and water fall actions

A character-capable branch now shares the existing fall algorithm: pilot protection, posture, heading, immersion, grouped damage and immediate flooding. The host fall action publishes private character injury feedback and newly lethal crew states, including nested flooding, atomically. Whole-unit character flooding can now follow flooded support into a fall and secondary cockpit breach. Tests cover dry-ground saved replay with RPG injury, preserved tactical counters, nested water casualties and rollback on evacuation failure. Ordinary fall/movement entry points remain gated; ice/bridge surface-break propagation and remaining airborne/normal combat publication are not claimed complete.

### Character surface-break actions and nested ice falls

Surface breakage now carries explicit casualty-publication capability through its shared terrain/occupant algorithm. Host ice/bridge actions select character falls for IC occupants and tactical falls for others; character fall checks propagate the same capability when they fracture ice. Tests cover mixed crews on ice and bridges, ordinary-call rejection, terrain/movement/notice rollback on failed evacuation, saved terrain and casualties, and neighbor-first nested ice fracture from a character fall. Ordinary movement, upward breakout and normal combat entry points remain separate integration work.

### Character upward ice breakout action

The explicit upward-breakout host action shares the existing breaker exclusion and altitude preservation with ordinary jumping. Neighboring character falls now publish injury feedback and evacuation atomically. Tests cover two water depths, breaker posture/structure/altitude preservation, neighbor-only falls, failed evacuation rollback, deterministic saved replay and persisted casualties. Connecting ordinary jump advancement and its surrounding movement transaction remains unfinished.

### Server airborne action and character free-fall impacts

The heartbeat now advances airborne events through a host action that preserves complete fall reports for private injury publication and casualty evacuation. Free-fall impact severity uses the signed shared fall resolver in character mode; direct lost-thrust, bridge and obstacle falls also select that mode. Tests cover surviving and fatal character impacts, ordinary-call rejection, failed evacuation rollback of health/dice/descent, saved replay and persisted crew loss. Existing TCP free-fall save/retry tests cover the enclosing server persistence boundary. Normal character jump launch and remaining landing/stacking/movement branches are not yet enabled.

### Airborne landing and ice-crossing propagation

Landing completion and vertical ice crossings now propagate the airborne action capability into ice fractures, neighbor falls and flooding. The action retains every nested fall for private injury feedback. Tests exercise upward crossing, landing before upward breakout, landing-induced fracture with character neighbors, and a character cockpit flooded on water landing. All cases check evacuation failure rollback, saved replay and persisted casualties. Character launch remains gated while stacking, physical collisions and interrupted ground movement are integrated.

### Character stacking collisions and avoidance

Stacking now retains applied impact and avoidance-fall reports for its host action and airborne callers. Character targets use ordered character impacts; tactical targets keep tactical injuries. The host publishes private direct/nested feedback and newly lethal crew evacuation. Tests cover mixed crews, surviving/fatal head injury without head destruction, both-unit damage, failed evacuation rollback, saved replay and avoidance falls. Airborne landing and obstacle stacking use this path; ordinary ground movement and physical DFA/charge publication remain incomplete.

### Character interrupted jump movement

The post-fracture horizontal ground segment now accepts the airborne action publication capability. Cliff/reverse falls, immersion checks and flooding propagate character injuries without losing their reports. A descending-ice regression covers a character jumper finishing in the next water hex, preserved submerged altitude, neighboring cockpit evacuation, complete rollback on failed evacuation, saved replay and persistence. Ordinary ground heartbeat updates and physical attack publication remain incomplete.

### Server ground movement action

The server now uses character-capable ground updates. Ground and airborne phases share the same report shape and host publication helper, covering water/ice entry, cliff/reverse falls, stacking injuries and new crew evacuation. Tests cover surviving water-entry falls, flooded cockpit death, position/altitude, evacuation rollback of movement/dice/health, saved replay and persistence. Existing server ground movement save/retry tests exercise the enclosing persistence boundary. Physical charge/DFA and firing paths remain incomplete; character jump launch is still gated.

### Character single physical attack action

Physical targeting and resolution now carry explicit host publication capability while sharing aim, hit tables, recycling, material damage and balance logic. Character targets receive ordered character impacts; failed balance uses character falls for the affected unit. Tests cover unauthorized cockpit rejection, surviving/fatal punch head injury without head destruction, failed evacuation rollback including attacker recycle/dice, saved replay and trip balance falls. This host operation does not yet route native/Lua commands or multi-arm sequencing, award physical XP, or enable charge/DFA character paths.

### Character multi-arm physical actions

The shared arm sequencer now carries character publication capability through each targeting check and completed attack. Host tests cover surviving two head hits, fatal health loss on either hit, rejection of the second arm after first-hit destruction, exact attack-dice consumption and recovery counts, failed evacuation rollback of both attacks, saved replay and persistence. Native/Lua command routing, physical XP and charge/DFA integration remain unfinished.

### Native/Lua character physical command routing

Physical command adapters now call the single/arm host actions without holding world borrows across publication or duplicating notices. A character punch parity test covers native and Lua state/output, private consciousness feedback, exactly one message per attack, fatal evacuation, persisted casualties and an outer Lua abort restoring both hits and crew movement. The fixture uses configured hit thresholds. Existing tactical command tests remain applicable. Physical XP, charge/DFA and firing integration remain unfinished.

### Direct physical piloting XP

Read-only characterization of `combat/mech_damage.c`, `combat/mech_physical_damage.c` and `character/character_experience.c` established pre-damage eligibility and `max(1, damage / 3)` piloting awards. Rust now applies these awards inside character-capable single/arm physical resolution. Tests cover direct kick/punch and glancing damage, miss/trip exclusions, friendly/tactical/disconnected exclusions, lethal-hit eligibility, failed evacuation rollback and saved XP. Charge/DFA awards and administrative XP-channel output remain separate gaps.

### Character one-way charge action

One-way charge profiles can now admit character participants within the host action. Ordered target/recoil packets use character continuation damage, preserving accepted packets after destruction; balance falls also use character health. Tests cover surviving and fatal-first-packet impacts, full packet counts, per-packet XP, exclusion of recoil and later wreck-hit XP, evacuation rollback and persisted state. Mutual charges, movement dispatch and DFA remain unfinished.

### Character mutual charge action

Mutual charge resolution now propagates character capability through both frozen profiles and prepared collisions. A regression kills the second pilot in the first collision and verifies that both accepted rolls/packet sequences still resolve, later pilotless attacks gain no XP, recovery applies to both, failed evacuation restores both collisions and XP, and final casualties persist. Ordinary movement dispatch and DFA remain separate integration gaps.

### Character charge movement dispatch

Charge endpoint dispatch now carries the movement action capability through one-way profile checks and both charge resolvers. Completed charge reports survive in the shared movement report until injury and casualty publication. A ground-dispatch regression covers one-way/mutual character casualties, expected intent cleanup, exact BTH notice counts, both-unit recovery, failed evacuation rollback, saved replay and persisted casualties. Airborne endpoint callers use the same dispatch mode. DFA and character launch remain integration work.

### Character DFA action

The DFA host action now retains character damage, failed-landing personal injury, balance falls and immersion consequences until atomic notification and casualty publication. Target packets use pre-impact physical XP eligibility; recoil and miss damage exclude XP. Regression coverage exercises fatal head injury before structural head destruction, completion of accepted packets, failed evacuation rollback of flight/dice/damage/recovery, character injury on a missed landing, deterministic replay and persisted results. Ordinary tactical APIs keep their character guards. Airborne DFA dispatch and character jump launch remain separate integration gaps.

### Character DFA landing dispatch

Normal and controlled early landing dispatch now select the character-capable DFA profile and resolver inside airborne host actions. The shared movement report retains DFA consequences for private injury feedback and casualty publication. A regression covers fatal target head injury during automatic landing, exactly one attack announcement, recovery/flight cleanup, failed evacuation rollback of the entire movement update, saved replay and persisted casualty state. Character jump launch remains gated.

### Character jump launch and command completion

Jump launch now admits character pilots and DFA targets. Launch itself only schedules a validated flight, so it needs no casualty publisher or duplicate host wrapper; the server's airborne action owns later damage and evacuation. Native/Lua parity coverage exercises bearing/range jumps, explicit DFA targets and selected targets from launch through landing, callback rollback of launch and output, identical final state/notices, and persistence. Pure world-only airborne simulation still rejects character damage when it cannot publish casualties. Character firing, thermal damage and other combat integration remain unfinished.

### Character stand actions and commands

Standing now has a character-capable host action sharing the existing eligibility, control roll and recovery scheduling. Failed attempts select character falls, retain nested injury feedback and publish casualties atomically. Native/Lua adapters call this action and avoid duplicate notices. Regression coverage includes successful rises, surviving failures, fatal failed-fall protection, no recovery timer after death, unavailable-afterlife rollback, outer Lua callback rollback, matching native/Lua state and output, and persisted results. World-only stand resolution preserves its tactical guard.

### Character timed stagger checks

The server now runs timed stagger through a host action that admits character units, shares history expiry and control checks, and resolves failed checks through character falls. The action publishes nested injuries and new crew casualties under one checkpoint; the server retains its outer persistence rollback. Regression coverage exercises Traditional, Consume and Retain histories with surviving/fatal falls, unavailable-afterlife rollback of history/dice/health, saved replay, exactly one cockpit stagger notice and persisted results. Pure tactical simulation retains its character exclusion.

### Explicit character ammunition detonation

An explicit detonation can now carry character capability into the existing ordered critical-loss and explosion cascade. Its host action publishes nested injuries and casualties atomically. Tests cover surviving/fatal crew injury with and without pain resistance, exactly one explosion injury application, spent-bin rejection, rollback of bin/material/health/dice state when evacuation fails, saved replay and persisted results. The thermal scheduler still needs to use this resolver together with character heat injury and shutdown falls.

### Character thermal action

Due thermal checks now have a character-capable host operation. Heat injury, ammunition detonation, airborne shutdown falls, crowding and zero-distance moving shutdown falls share their existing ordered resolver with explicit character publication capability. Reports retain character injuries and stacking consequences. Tests cover fatal heat injury preventing subsequent ammunition/shutdown checks, fatal ammunition injury preventing shutdown, ground/air shutdown falls, failed evacuation rollback of due clocks and damage, deterministic replay and persisted results. The server still uses the tactical thermal entry point; connecting the host action while preserving staged message ordering remains integration work.

### Server character thermal dispatch

The server now runs thermal checks through the character-capable host action. Earlier accumulated cockpit and unjam feedback is staged before thermal publication; duplicate deferred thermal messages were removed. A real server regression injects a failed database write for fatal character heat damage and verifies unchanged persisted health, clocks and crew location, followed by a successful retry matching direct thermal resolution. The server's outer transaction retains responsibility for rolling back later phase or persistence failures.

### Character grouped weapon damage

Salvo resolution now distinguishes material-only, tactical and character-capable effects while sharing cluster rolls, location selection and group ordering. Character weapon impacts preserve ammunition-specific effects in the existing cascade. A host operation publishes each group's injuries and nested falls before casualty transfer. Tests cover a surviving four-missile salvo, fatal first-head-hit termination without structural head destruction, evacuation rollback of cluster/location dice and damage, saved replay and persistence. Shot admission, expenditure, recoil/misload and command routing still need character integration; this operation handles successful grouped hits only.

### Character direct shot action

Direct shots now carry character capability through admission, grouped target damage, internal misloads and Heavy Gauss recoil falls. The character shooter's Toughness is derived separately from target Toughness. A host action publishes shot consequences and new casualties under one checkpoint. Laser/SRM regressions cover live and fatal character head hits, full surviving missile salvos versus first-hit termination, ammunition/recycle publication, failed evacuation rollback of the complete shot, saved replay and persistence. Native/Lua firing adapters and shooting XP remain unfinished; character-specific misload and recoil regression coverage is still needed.

### Native/Lua character firing commands

Configured firing now resolves character shots inside a host checkpoint while retaining pre-damage observer selection and existing aim, misload and weapon feedback. Both adapters use the shared publisher; Lua no longer sends a second copy through its own outbox path. Regression coverage compares native/Lua laser and missile firing for surviving/fatal head injuries, exactly one firing announcement, private consciousness feedback, callback/evacuation rollback and persistence. Shooting XP and dedicated character recoil/misload tests remain gaps.

### Character shooter recoil and misload regressions

Dedicated tests now cover surviving/fatal Heavy Gauss recoil and a head-mounted AC/2 catastrophic rapid-fire misload. Recoil verifies shooter health, target-Toughness isolation, saved replay, ammunition expenditure and fatal evacuation rollback. Misload uses the normal rapid-fire control, verifies an unlaunched shot with one character head injury, two consumed rounds, an unchanged target, weapon/bin/health rollback and persisted casualties. Shooting XP remains unfinished.

### Classic gunnery XP calculation

Read-only characterization of `character/character_battle_value.c` and the pre-damage call in `combat/mech_damage.c` established two distinct configured formulas. Rust now provides the biped classic calculation selected by default (`oldxpsystem`): integer bounded tonnage ratio, cargo-adjusted speed bands, losing 2d6-outcome weighting, per-unit scaling, inclusive 1..50 gate, and 1..50 award clamp. Unit tests cover probability boundaries, scaling, caps and invalid inputs. This calculation is not yet invoked by shots. Remaining work includes active-gunner/target eligibility, persisted per-unit suppression/modifier settings, award RNG and transactional skill mutation. The alternate formula requires battle-value computation and its configuration multipliers; it must not be approximated by the classic formula.

### Persisted unit XP settings and gunnery eligibility

Constructed units now persist a multiplier (default one) and gunnery-suppression flag, validated before arithmetic or publication. Read-only eligibility covers assigned/present connected gunners, character participants, self/friendly/dead targets and formula-specific difficulty/suppression rules. The classic formula intentionally ignores suppression, matching its distinct reference branch. Tests cover eligibility without RNG mutation, saved settings, rejected invalid updates and invalid persisted scaling. Shot award mutation, RNG and alternate battle-value computation remain unfinished.

### Atomic classic gunnery awards

A classic award operation now combines pre-impact eligibility, persisted per-unit scaling, weapon skill selection, an attacker-owned persisted 1..50 roll and ordinary skill XP mutation. It returns rejected random gates as reports and consumes no dice for ineligible hits. Effective speeds are explicit inputs because the reference speed term includes load, equipment and gravity; the operation does not substitute nominal or current travel speed. Tests cover exact RNG consumption, extended/general skill selection, zero/disabled unit scaling, save/load replay and rollback of RNG when an award fails for missing attributes. Shots still need to supply effective-speed policy and invoke awards between damage groups; alternate battle-value XP remains unfinished.

### Derived effective speed for classic gunnery XP

Classic awards now derive both speed inputs from the world. The shared unit calculation uses live fixed-point mass, underweight/overweight normalization, a strict three-times-weight cutoff, template maximum, sampled TSM and special-map gravity with a 50-percent floor. Tests cover weight boundaries and overflow, heat thresholds, disabled/enabled gravity, limb/ammo loss, read-only calculation and saved replay. This is the supported unloaded biped path; cargo/towing and sprint/MASC/supercharger state remain unimplemented. Movement throttle and shot award invocation are separate unfinished integration work.

### Classic gunnery XP in character firing

Configured character shots now supply pre-impact identity, BTH, weapon, timestamp and scaling policy to the shared grouped-damage traversal. Each group awards classic XP after its location roll and before damage; fatal groups receive their award and terminate subsequent groups. Reports retain optional attempts aligned with applied groups. Extended laser/SRM action and native/Lua regressions verify awards, exact attacker dice, saved replay, final-group death, evacuation/callback rollback and persisted state. Independent replay comparisons ignore only wall-clock skill timestamps. A policy regression covers zero per-unit scaling, disabled scaling, friendly hits and misses. The game fixture explicitly selects `oldxpsystem = 0`, so classic tests opt into that formula; battle-value XP is still unsupported and is not replaced by classic awards. Heat-only/coolant fire and internal shooter failures remain outside target material-damage XP. Administrative XP-channel notifications remain unfinished.

### Conventional biped Battle Value

The Rust weapon catalogue now includes intrinsic BV for all 124 supported identities. A typed unit calculation derives offense using descending weapon BV and an integer half-value heat budget, plus tonnage. Defense includes current armor/structure, installed engine/gyro factors, per-bin ammunition and per-slot Gauss exposure, CASE, effective running MP, installed TSM conversion and the independent jump bonus. It retains single-precision defensive rounding. Supported equipment currently excludes defensive electronics/AMS and sprint/MASC/supercharger state. Tests characterize Jenner/Atlas totals, current armor versus retained destroyed weapons/empty bins, jump losses, CASE and parent-torso Gauss containment, engine variants, hardened-gyro mass effects, map gravity and hot/cold TSM. The operation is read-only and round-trips through unit serialization. Battle-value gunnery XP remains to be integrated with pilot/configuration modifiers; this score is not a claim of support for unimplemented classes or equipment.

### Battle-value gunnery XP integration

The formula selected by the shared game now awards before each character weapon group through the same dispatcher as classic XP. It uses current BV, nominal speed MP, the attacker weapon family for both pilots, optional piloting/gunnery BV modifiers, literal configuration multipliers, BTH weighting, recycle scaling, per-unit scaling and the configured cap. It consumes no XP dice and honors suppression. Formula tests cover switches, caps, zero values, truncation, eligibility and invalid configuration. Undefined square-root/nonfinite results from poor-skill extrapolation explicitly receive one XP and an absent difficulty; this defines an edge where C floating-to-integer conversion is undefined. Direct and native/Lua laser/SRM regressions now run both formulas with trained crews, verify decreasing BV difficulty between surviving groups, fatal-group termination, saved replay and whole-shot rollback. A dedicated atomic award regression checks finite pilot adjustment, default unpiloted target skills, no dice, persistence, suppression and invalid-setting rollback. Administrative XP channels and unsupported equipment/classes remain outstanding.

### Shooting XP diagnostic channels

A typed diagnostic record now captures accepted XP wording and participant names before each damage group. Character shot hosts publish those records through the existing communication service under their world/effects checkpoint. Missing channels remain silent, newline text is flattened, and channel history/receive policy are shared with ordinary communication. Both-formula native/Lua regressions now include subscribed diagnostic channels, exact message counts, recipient filtering, saved channel history, Lua-abort rollback, channel-counter failure and rollback after fatal evacuation failure. A direct-action regression verifies the noisy battle-value trivial-hit line and suppression precedence. Classic rejected gates and rate-limit rejections emit no gain record. Shooting diagnostics are integrated; other diagnostic call sites remain unfinished.

### Physical XP skill selection

Read-only review of `find_piloting_skill_name` identified a hard-coded general-skill award. Physical, charge and DFA awards now share the biped piloting-skill selector with skill checks and receive the existing extended-piloting rule. Eligibility/glancing tests now run both settings and check the selected ledger while preserving the other ledger. Charge tests cover both settings and saved fatal-hit rollback; DFA tests now include an eligible character attacker, one award before the fatal first group, both skill settings and saved replay. Native/Lua two-arm punch tests toggle the real configuration, verify two XP in the chosen skill, and retain abort/casualty/persistence coverage. Piloting XP channel publication remains a separate unfinished call site.

### Physical piloting XP diagnostics

Accepted physical, charge and DFA awards now snapshot `MechPilotXP` messages before damage. Direct host actions, native/Lua physical commands and movement-dispatched collisions publish those reports through the shared transactional channel service. Missing channels are ignored; delivery failures restore channel counters/history, XP, combat state and pending output. Regressions cover both configured piloting skills, two-arm publication exactly once per accepted award, failure on the second message, casualty rollback, movement replay and persistence. Movement-check position-mark XP and other diagnostic call sites remain unfinished.

### Movement piloting XP cadence

Character ground and ordinary jump movement now share a persisted count of successful hex-changing updates. Every tenth update offers one piloting XP when an active connected pilot is present and the destination coordinates differ from the last eligible attempt. The mark starts at `(0, 0)` and deliberately excludes map identity; disconnected crossings still advance the count without consuming the mark. Movement reports stage accepted `MechPilotXP` messages under the host checkpoint. Tests cover the full tenth-crossing boundary, both skill settings, same-coordinate suppression, disconnected/tactical participants, airborne entries, save/load before the award and channel failure restoring the whole movement tick. Interrupted ice synchronization also records accepted crossings; detailed hazard-specific XP characterization and the distinct XP-awarding control-check call sites remain to be audited. Standing uses the reference's no-XP control check.

### Physical and collision control-check XP

Character physical, charge and DFA host actions now apply the successful piloting-check award separately from damage XP. Actual successes with target above two earn `clamp(target - 7, 1, max(2, 1 + modifier))`; automatic prone successes, failed rolls and trivial targets earn none. Eligible crew receive the configured piloting skill award without consuming dice or changing movement progress. `BattlePilotingCheck::experience` retains the skill mutation, while the enclosing attack report retains the diagnostic for transactional publication. Tests cover formula boundaries, trip eligibility, both skill modes, native/Lua and callback rollback, charge movement replay, hit/missed-DFA protection, channel failure and persistence. Other successful-check call sites (fall protection, landing, stacking, damage balance, heat and terrain) still need integration; no-XP checks such as standing remain distinct.

### Fall protection XP and nested publication

Character falls now apply the shared successful-control XP rule immediately after the protection roll, before personal or unit damage. The fall report retains the award on `avoidance.experience` and captures accepted diagnostics in `experience_messages`. Shared host consequence traversal publishes these for direct falls and nested physical, collision, shooting, flooding and terrain reports under the enclosing checkpoint. Tests cover both skills, failed/trivial/prone/disconnected checks, save/load replay, rejected channel delivery and a failed trip balance followed by successful protection with exactly one message. Existing charge regressions distinguish balance XP from subsequent fall protection XP. Landing, stacking, damage balance, overheat and terrain control checks still require their separate successful-check award integration.

### Damage-triggered balance XP

Immediate balance checks caused by gyro/leg critical damage now apply the shared successful-check XP policy. `BattleBalanceReport` retains the award on its check and accepted diagnostic snapshots; impact, grouped-shot and nested-fall publishers deliver those messages under the owning transaction. Forced falls still skip balance XP and may independently earn fall-protection XP. A penetrating-hit regression covers both piloting skills, exactly one message, disconnected crew, saved replay and rejected channel delivery restoring damage, critical losses, dice and XP. Landing, stacking, overheat, stagger and terrain control-check callers remain to be integrated.

### Stagger and thermal shutdown control XP

All three stagger-history modes now apply the successful-control XP policy before resolving a possible fall. Thermal shutdown likewise awards a successful moving-pilot balance check before powering down. Their reports retain check mutations and diagnostic snapshots, published inside their existing host checkpoints. Tests cover both skill settings, all stagger modes, history rollback, no repeat award on the following stagger tick, disconnected pilots, saved replay and thermal power-down rollback. The thermal regression preserves the untrained Computer override's three-die sequence. Landing, stacking and terrain control checks remain separate integration work.

### Crowding avoidance XP

Crowding avoidance checks now apply successful-control XP and retain accepted diagnostics with the collision effects. Direct crowding actions, ground movement and landing publishers share that path; thermal fall handling also carries the diagnostic collection when unpacking collision effects. Tests cover ground, jump and fall inputs, both piloting skills, disconnected crew, unchanged avoided targets, persisted awards and channel failure restoring motion, dice and XP. Damage-mode collisions still use their own damage/balance rules without an additional avoidance award. Landing-specific and terrain-specific control checks remain unfinished.

### Automatic damaged-gear landing XP

Automatic landing now captures successful damage-related piloting-check XP in its landing result and merges those diagnostics into the airborne host report. Normal completion, ice completion and obstacle-abort completion share the result path; existing fall and collision owners retain their separate effects. Tests cover actuator, hip and gyro damage, both piloting skills, restart during flight, exactly one accepted diagnostic, final persistence and delivery failure restoring the unfinished landing. Early manual landing still uses a separate tactical adapter and requires character-mode/XP integration. Landing stagger checks and terrain-specific checks remain to be audited.

### Character-aware manual early landing

Native `land` and Lua `btech.unit.land` now use one host landing transaction shared with timed movement publication. Early-abort XP, damaged-gear landing checks, DFA, crowding, flooding, falls and new crew casualties retain their reports until publication. The pure-world landing operation explicitly rejects character units; `land_battle_jump_action` provides the host-capable Rust entry point. Tests compare native/Lua output and state for success, damaged gear, surviving failed landings and fatal falls in both skill modes, including callback abort, failure on the second XP message, failed evacuation and persistence. All existing tactical jump/landing tests also pass. Landing stagger and terrain-specific rules remain to be audited.

### Water-entry and reverse-slope XP

Character water-entry and reverse-slope control checks now apply successful-check XP. Ground segment results retain their diagnostics, including segments used to finish interrupted ice jumps, and the movement host publishes them transactionally. Reverse-slope success continues to skip the separate water check. Tests cover ordinary water, reverse uphill and reverse water entries, both skills, exact single-roll consumption, persisted awards and rejected channel delivery restoring movement and XP.

A read-only landing-stagger audit found that `mech_condition_state.c` reads a separate persisted `stagger_damage` counter, while ordinary damage appends to the rolling list or traditional turn total. `mech_stagger.c` exposes that counter separately; it must not be inferred from the Rust rolling-history sum. Support for externally restored positive values of that separate counter remains unfinished. No extra landing roll has been introduced from unrelated damage history. Cliff-check policy and remaining control callers still need auditing.

### Heavy Gauss recoil XP

Moving character shooters now apply successful-control XP to the Heavy Gauss recoil check, including when the shot misses. The recoil report retains the award and diagnostic; shot publishers deliver it within the existing action checkpoint before any nested fall diagnostics. Tests cover both piloting skills, moving/stationary fire, disconnected and tactical shooters, a missed shot, saved awards and channel failure restoring ammunition, heat, dice and XP. Cliff avoidance remains a no-XP check, as specified by the reference's explicit `mech_pilot_skill_roll_without_experience` calls. Obstacle-jump and weapon-unjamming XP callers still require integration.

### Obstacle recovery and timed unjamming XP

Obstacle-jump recovery now awards successful control XP before completing the landing, preserving any separate damaged-gear landing award. The landing regression covers both skill modes, actuator/hip/gyro damage, restart during flight and failure on the second diagnostic restoring the whole landing.

Timed non-rotary unjamming now stages successful-check XP with recovery feedback. The server and public `advance_battle_unjamming_action` share resolution and publication; pure-world advancement rejects active character attempts. Tests cover both skills, final-second restart, failed/prone/disconnected/tactical checks, empty supply, rotary recovery without control XP, channel failure restoring the timer and discarded round, and an authenticated server session retrying after a rejected database commit. Standing and cliff avoidance remain explicit no-XP callers. The separate persisted landing-stagger counter and unimplemented equipment/unit classes remain outside this completed caller audit.

### Conventional anti-missile defense

IS and Clan ammunition-fed AMS are now typed equipment with persistent controls, automatic interception and defensive Battle Value. The shot transaction owns defensive heat, ammunition, recycle and dice; salvo grouping receives only the intercepted count. Tests cover successful and missed shots, short/empty supply, disabled/shutdown/recycling/destroyed/flooded systems, non-missile immunity, LRM regrouping, Streak success/failure, native/Lua parity, callback rollback and persistence. The catalogue now has 126 supported weapon identities. Laser AMS and interactions with unimplemented pod/swarm/mining weapons remain unfinished, along with the other deferred equipment and unit classes.


### Laser AMS and mixed defenses

IS and Clan laser AMS now share automatic defense, control, critical-loss and persistence behavior. Reference-checked profiles retain matching-bin requirements, 25-second recycle, distinct heat costs and zero positive ammunition BV. Tests extend all eligibility and native/Lua scenarios to four AMS identities, verify installations without bins, and cover mixed-mount ordering and whole-system capability loss. The weapon catalogue contains 128 identities, all represented in the checked Lua weapon union. Pod/swarm/mining interactions and other unimplemented equipment and classes remain pending.

The same audit corrected targeting-computer eligibility for conventional AMS.
Laser AMS retains beam-family eligibility for equipment sizing. Catalogue and
Clan energy regressions cover that distinction without enabling manual fire.

### CASE II

`CASE-II` criticals parse as `BattleSystem::CaseIi`. It is a noncritical slot, like
CASE, and weighs one ton per Inner Sphere slot or half a ton per Clan slot. It costs
175,000 per slot, matching the reference cost table. The reference defines a CASE II
section bit but never applies it, so the combat rule follows the published rules
instead of the C code.

When ammunition or a weapon explodes in a CASE II location, the section takes one
point of internal damage with its normal critical roll. The rest of the blast goes
to that section's armor (rear armor on torsos), and damage beyond that armor is lost
without transferring. The pilot takes one injury instead of two, reported as
`BattleImpactEffect::VentedExplosionInjury` when no tactical rules are supplied.
CASE II also counts as CASE for containment and for vehicle power-plant containment.
Battle value drops the ammunition and Gauss exposure penalties for a CASE II location,
including the center torso, head, legs and XL side torsos. Tests cover the venting
path, battle value, mass and parsing.

### Chassis technologies

`BattleTechnology` names seven chassis flags and accepts either the reference's full name
or its abbreviation, as the reference template loader does. Cost helpers accept both
spellings too.

- **Hardened armor** (`HardenedArmor_Tech`/`HARM`): conventional Mech armor damage is
  halved, rounding up, before armor absorbs it, and overflow stays halved. This matches
  the reference damage code and the existing vehicle rule. Armor weighs eight points
  per ton.
- **Reinforced structure** (`ReinforcedInternal_Tech`/`RINT`) halves internal damage,
  rounding up. **Composite structure** (`CompositeInternal_Tech`/`CINT`) doubles it.
  Overflow keeps the modified value, as in the reference. Reinforced structure weighs
  twice as much as standard, and composite weighs the same as Endo Steel. Limb loss on a
  critical roll of 12 still destroys the whole location.
- **Small cockpit** (`SmallCockpit_Tech`/`SMCPIT`): both spellings give the two-ton
  cockpit, the +1 piloting modifier and the 175,000 cost.
- **Laser heat sinks** (`LaserHS_Tech`/`LHS`): the reference marks these as
  unimplemented, so tabletop rules apply. Laser heat sinks are Clan equipment, so
  dissipation and slots follow the chassis sink rules (double heat sinks on Clan
  chassis). A running unit that carries them glows, counting as illuminated in darkness.
- **Watchdog CEWS** (`WatchDog_Tech`/`WDOG`): unimplemented in the reference, so tabletop
  rules apply. The unit's installed ECM slot also works as an active probe
  (`BattleActiveProbe::Watchdog`, Clan active-probe reach), and damage to that slot
  disables both functions. The slot weighs a ton and a half and costs 500,000.
- **Artemis V** (`ArtemisV_Tech`/`AV`): unimplemented in the reference, so tabletop rules
  apply. The unit's Artemis controllers add three to the cluster roll instead of two, and
  Artemis rounds get a -1 to-hit modifier. ECM that blocks Artemis IV also blocks both
  effects. Narc homing keeps its ordinary bonus. A controller weighs a ton and a half and
  costs 250,000.

Tests cover Mech damage and mass for every spelling, laser heat sink glow, the Watchdog
probe's reach, damage and restart, and the Artemis V cluster, aim and mass rules.

### Conventional Narc beacons

IS and Clan Narc launchers now use native Rust pod resolution, distinct explosive
ammunition, compatible missile controls, AMS interception and persistent section
marks. Pod hit locations transfer off destroyed sections and retain Exile cockpit
stun without applying armor damage or damage XP. Destruction removes the affected
mark. Compatible missile guidance shares the existing cluster bonus calculation;
beacons do not expire or have team ownership. Native and Lua actions share the shot
transaction and expose detached pod receipts. The catalogue contains 130 identities.

Regression coverage includes both launcher profiles, attachment, miss, interception,
explosive damage, stun, section transfer/removal, guidance, persistence and native/Lua
callback rollback. iNarc and ECM suppression of guidance remain unimplemented, along
with the other deferred equipment and unit classes. This completes conventional
Narc behavior for the currently supported equipment, not the entire integration.

### Electronic field calculation

The Rust electronic-warfare domain now resolves Guardian, Angel and personal
ECM/ECCM contributions through one field calculation. It covers inclusive six-hex
and half-hex boundaries, opposing-team cancellation, double Angel strength,
independent protection/disturbance, self-interference contributions and transition
notices. A continuing disturbance retains its ordinary/Angel classification until
clearance, matching the reference's observable status transitions. Mode selection
is exclusive, and field state can be serialized without storing emitter lists.

This is the calculation layer for the remaining electronics integration. Equipment
construction, mass/BV, live suite controls, shutdown/critical handling, periodic
world-field refresh and missile/sensor consumers are still pending. No ECM equipment
has been newly accepted by template activation, and Narc/Artemis suppression is not
yet connected to this calculation. The next step is to supply field inputs from
validated equipment and committed map/unit state, then wire consumers and adapters.

Read-only behavioral sources: `sensors/mech_ecm.c` (range, strength and transitions),
`unit/mech_condition_state.c` (exclusive operating modes),
`unit/mech_lifecycle.c` (power-down mode clearing), `unit/template_specials.c`
(Guardian presence and two functional Angel slots for BattleMechs),
`combat/crit_mechs.c` (suite loss), and `combat/mech_combat_missile.c`
(target protection/shooter disturbance suppress guidance). These are specifications
for the Rust implementation; no reference-tree files were changed.

### Installed Guardian and Angel electronics

Guardian `Ecm` and two-slot biped `AngelEcm` now activate native Rust electronic
controls. Mode state and field observations persist with units. Controls require
a conscious assigned pilot and a running, placed unit; shutdown, suite critical
loss, section loss and flooding disable affected emissions. The server refreshes
fields as part of its existing atomic heartbeat, and current same-map spatial
fields drive direct-shot guidance even before an observation refresh.

Native `ecm`, `eccm`, `angelecm`, `angeleccm` and matching Lua unit methods share
control and notification rollback. Status and Lua inspection expose electronics.
Mass uses 768 units per IS Guardian slot, 1,024 per Clan Guardian or Angel slot;
Guardian BV adds 61 once for a complete installation (two IS slots or one Clan
slot). Angel has no positive defensive BV in the reference calculation.

Narc and Artemis cluster bonuses are suppressed by target protection or shooter
disturbance. Angel interference also forces Streak launches despite failed locks
and uses ordinary SRM cluster hits, while Guardian interference retains Streak
homing. Tests cover equipment accounting, mode exclusivity, native/Lua parity,
rollback, damage and shutdown, flooding, map/range separation, guided salvos,
Streak hits/misses/glances and the actual server heartbeat saving field observations.

Personal ECM equipment, stealth/iNarc sources, advanced sensors, C3/C3i and other
unimplemented electronic consumers remain pending. The earlier standalone-field
entry describes the preceding milestone; Guardian/Angel runtime integration and
Narc/Artemis suppression are now connected.

### iNarc pod effects and shared beacon storage

The IS iNarc launcher now supports homing, explosive, haywire, ECM and Nemesis
ammunition selections through `inarc` and Lua. The checked catalogue profile is
five tons, three slots, four rounds per ton, six explosive damage, one heat,
4/9/15 range and 30-second recycle. The catalogue contains 131 identities.

Attached effects now use one section-to-kind-set representation for conventional
Narc and iNarc. Hit location, dead-section transfer, cockpit stun, interception and
section destruction are shared. Homing assists Narc-compatible attack accuracy
by one and shares its cluster bonus with ordinary Narc. Haywire adds one to weapon
attack difficulty. ECM pods supply the shared field calculation's 1,000-point
self-interference contribution; they do not emit a field around nearby units.
Explosive pods use ordinary salvo damage. The reference accepts Nemesis ammunition
but resolves its attachment as homing; this behavior is preserved without inventing
missile redirection.

Tests cover all ammunition selections, matching-bin exhaustion, hit/miss/AMS
outcomes, coexisting effects, aim and non-stacking guidance, saved state, destruction,
native/Lua parity and complete callback rollback. Stealth/personal electronics, advanced sensors and other deferred systems remain
unfinished. The Lua unit view exposes `beacons` and the conventional-only
`narc_sections` query; pod receipts identify their `kind`.


### Pod inspection and biped removal

`pods` and `btech.unit.pods` inspect attached effects without changing state.
`removepod <location> <type>` and its Lua counterpart remove one iNarc effect:
Y selects haywire, E selects ECM, and other selectors choose homing. Conventional
Narc remains attached. Arm pods require the opposite arm; other locations use the
least impaired available arm, preferring the left on ties. Destroyed or recycling
arms and arms with recycling weapons cannot swat. Missing upper/lower actuators
and hands increase difficulty; upper/lower damage also halves failed-swat damage.
Both outcomes recover the chosen arm for 60 seconds. Failed swats use the shared
impact, injury, fall and casualty handling. Raw swat rolls grant no control XP.

Successful removal preserves other effects and refreshes electronic fields.
Tests exercise arm selection, actuator losses, recovery, self-damage, permissions,
coexisting pods, ECM clearance, persistence, native/Lua parity and callback rollback.
Vehicle crew removal remains pending with unsupported vehicle classes.

### Stealth armor

Stealth armor is now represented by passive `StealthArmor` critical slots. A biped
needs two in each arm, leg and side torso, plus Guardian ECM; Angel equipment alone
is insufficient. The template flag is accepted, but capability is derived from
parts. Armor slots add no equipment mass and cannot receive random critical hits.

Native `stealth` and Lua `btech.unit.stealth(unit, pilot)` schedule a persisted
30-second switch. Repeated requests cannot replace it. The event retains its
requested destination through shutdown; it consumes without switching if power or
Guardian capability is unavailable when it expires. Active armor clears on power
loss or Guardian damage. Active armor adds ten continuous heat and shares the
1,000-point self-interference contribution with iNarc ECM (not stacked). It raises
medium/long/extreme weapon range penalties to 3/6/12 without changing reach,
short/minimum penalties, or optical visibility. The server advances the switch and
publishes field observations in its existing commit transaction.

Tests cover equipment layout, accounting, passive criticals, range boundaries,
actual weapon previews, delays, damage/shutdown, persistence, native/Lua parity,
callback rollback and a real server switch saving the resulting interference.
Advanced probe detection, C3, null signature systems and personal ECM remain
pending with their corresponding sensor/equipment work.

Firing at an active stealth target requires a settled lock on that unit. Missing,
settling or different-target locks reject the shot before ammunition, heat,
recovery or attack dice change. Normal visibility and firing checks still apply.

### Null signature system

`NullSig_Device` is a critical-vulnerable, one-ton device. A biped needs at least
one in each section except the head; a technology flag alone does not grant the
capability. Loss or flooding of any installed device disables the whole system.
Native `nss` and Lua `btech.unit.nss(unit, pilot)` share the thirty-second saved
switch mechanism with stealth armor, using `BattleSignatureState` and
`BattleSignatureTransition`. Each system retains independent selection and events.

Active NSS contributes ten heat and the same non-stacking range penalty as stealth.
It supplies no ECM self-interference and does not impose stealth armor's stable
firing-lock requirement. Shutdown and device loss clear its active effect; pending
events are consumed if unavailable at expiry. Native status and Lua inspection show
both selections. Tests cover full/incomplete equipment, mass, critical exposure,
damage, shutdown, expiry, persistence, coexistence, unlocked firing, native/Lua
parity, rollback, and a server heartbeat committing a saved switch.

Advanced probe and infrared detection remain pending with the corresponding sensor
implementation; NSS state is available for those consumers.

### Infrared sensors

`I`/`infrared` extends the existing optical sensor selection and contact pipeline.
It operates to fifteen hexes, sees through smoke and water, and rejects fire,
blocked terrain and six intervening woods. Acquisition uses `80 - range` before
existing pilot/arc/secondary weighting. Lighting and weather visibility do not
change its hardware range, but the map's saved maximum visibility and infrared
disable bit still apply. Daylight reconciliation preserves infrared selections.

Aim uses four-thirds of total woods, three for partial cover, and signed thermal
contrast: +2 at nonpositive contrast, +1 through 20, zero through 35, -1 through
50 and -2 above 50. Contrast is `2 * (production - cooling) + min(production, cooling)`; production includes stored weapon heat and active
concealment heat. Sensor reports and selected aim contributions now retain signed
values throughout. Native/Lua selection, unit state and independent map controls
accept the new mode. Tests cover limits, obscurants, thermal boundaries, actual
weapon preview bonuses, daylight switching, persistence and callback rollback.

Other sensor families and unsupported unit-class-specific rules remain pending.

### Seismic rule foundation and shared sensor types

The shared public types now use `BattleSensorMode`, `BattleSensorReport`,
`BattleSensorAim`, and related `BattleSensor*` names. Existing optical queries and
Lua values retain their behavior; no compatibility aliases are introduced.

`BattleSeismicRules` evaluates an explicit signal-strength observation and 0/1
attack adjustment. Its guaranteed four-hex reach expands in twenty-point signal
bands to a hard eight-hex limit. It requires a running, non-jumping target and a
non-jumping observer. Unless configured to detect stopped units, target speed must
strictly exceed one MP in either direction. Its aiming movement bonus instead
includes the one-MP boundary. Aim uses truncated current physical tonnage, partial
cover and the explicit attack adjustment; terrain obscurants do not block it.
Acquisition strength is `50 - 4 * range` before existing detection weighting.

The read-only `battle_seismic_contact` world query uses owned power, motion, mass,
map limits and the independent seismic disable bit. Tests cover signal-band edges,
power/jump/motion boundaries, mass thresholds, cover, invalid inputs, persistence,
and unchanged dice/state during inspection.

Player selection is not enabled yet: saved signal fluctuation and transactional
random aiming contributions must be connected first. This is the rule foundation,
not completed runtime seismic sensing. Other sensor families remain pending.

### Runtime seismic sensing

Seismic is now selectable with `sensor S S` and Lua `btech.unit.sensors`. It uses
the ordinary delayed selector, contact scanner, current-contact checks and firing
pipeline. The runtime sensor policy reads `battletech.seismic_see_stopped` when a
Lua host starts or is reconfigured; all query consumers use the same snapshot.
Database loading leaves this runtime policy at its default until host configuration
is installed. Scenario callers may set it explicitly through the domain API.

Every running live unit advances a saved signal by a uniform -40 through +40,
clamped to 0–100, on the one-second heartbeat. Signal fluctuation uses a separate
saved stream, so background updates do not consume combat dice. Unit inspection
exposes strength without revealing the stream. Both strength and stream roll back
with a failed server save.

Eligible seismic aiming samples its 0/1 adjustment on an attack candidate. A
preview uses a cloned stream; firing retains that candidate stream before rolling
the attack. A duplicated primary/secondary mode samples once. Different eligible
modes are compared with the ordinary secondary penalty. Rejected attacks and Lua
callback failures preserve the attack stream and expenditure. The earlier rule
foundation is now connected to runtime selection and simulation.

Tests cover signal replay, shutdown, independence from attack dice, sampled preview
and shot agreement, configuration, native/Lua switching, callback rollback, and an
actual rejected server save followed by successful signal/contact retry.

### Electromagnetic sensors

`E`/`electromagnetic` uses the shared saved signal to vary reach from sixteen to
twenty-four hexes. Mountains, blocked terrain, eight intervening woods points and
current ECM disturbance on the observer prevent detection. Fire, smoke and water
do not independently block EM. Acquisition strength is `30 - range` before the
existing detection weighting. The map's EM disable bit and maximum visibility
remain authoritative; eligibility queries use current ECM, not stale field caches.

Aim uses two-thirds of total woods, partial cover, nominal tonnage, movement and
recent firing. Heavy targets are easier to hit, movement at or above one MP in
either direction adds one, and firing since the preceding heartbeat subtracts one.
The shared transactional sensor-jitter path supplies an additional zero or one.
Native/Lua switching, contact tracking and weapon aim all accept this mode.

A launched shot sets `fired_recently`, including a miss; rejected shots and failed
Streak locks do not create the marker. The heartbeat clears it before movement,
heat and scanning, and failed persistence restores it. Unit inspection exposes
the saved marker. Tests cover range bands, obstacles, ECM blocking and recovery,
nominal mass thresholds, movement, firing emission, preview/shot agreement,
persistence, native/Lua parity, callback rollback and heartbeat retry.
Active probes are described below.


### Radar sensors

Radar is available through native and Lua `R`/`radar` selection on AntiAircraft
chassis. Selection and restored active/pending pairs enforce equipment requirements.
Current jump/fall altitude, terrain clearance, strict low-altitude range, the
180-hex hardware limit and altitude-eleven extended scan range are integrated
with the shared contact and aiming pipeline. Radar uses map disable bit 32,
ignores independent smoke/fire/ECM obstruction, and retains blocked terrain.
Signed aim includes woods, partial cover and the altitude-ten bonus.

Tests cover range/acquisition edges, sensor surface datums, invalid inputs,
equipment rejection without mutation, saved pending selection, map-bit isolation,
read-only queries/aim, native/Lua parity and shot rollback. Aircraft remain unsupported.


### Active probes

Beagle, light Beagle and Bloodhound equipment, native/Lua mode selection, current
ECM/concealment eligibility, acquisition and transactional aiming are integrated.
The families have six/three/eight-hex reach, ignore terrain and cover, and use
zero-to-two aiming jitter. Acquisition 101 bypasses hidden-target penalties while
retaining arc and secondary weighting. Component loss disables the family and
returns its active slots to visual; pending damaged requests expire without activation.

Tests cover range boundaries, signature penetration, Angel protection, equipment
mass, missing equipment, map disabling, damage fallback, persistence, hidden-target
acquisition weighting, firing through blocked terrain and native/Lua rollback.
Infantry-specific probe exclusions remain outside supported unit classes.

### Airborne forest aiming

The shared unit terrain report removes target-hex woods above two levels over the
forest's ground elevation. It uses integer jump/fall altitude, so visual,
light-amplification, infrared, EM and radar share the same canopy boundary. Radar
no longer adjusts target woods separately. Regression coverage checks both flat
and raised forests, the inclusive two-level boundary, per-sensor aim differences
and read-only query behavior.

### TAG targeting lifecycle

TAG equipment, acquired enemy-only range/terrain admission, thirty-second lock/recycle
cadence, unique target ownership, persistence, status, native/Lua controls and
heartbeat cleanup are implemented. Current ownership queries exclude damaged,
stopped, removed, blocked and out-of-range links before cleanup. Tests cover
restart at the lock boundary, repeated-tick notices, invalid timers/ownership,
no-dice behavior, interruption, takeover, rejected targets and Lua rollback.
Semi-guided ammunition's TAG consumer is described below.

### Semi-guided ammunition and TAG assistance

Semi-guided templates/bins, live native/Lua controls, matching ammunition supply,
compact status, persistence and the TAG movement-modifier consumer are integrated
for supported indirect-fire missile profiles. Friendly TAG from another unit
suppresses positive movement penalties, including during lock settling, without
removing negative modifiers. Lost/self/enemy links restore ordinary movement aim.
Tests cover all supported ammunition profiles, capacities, saved state, current
TAG changes, aim purity, native/Lua parity, callback rollback and empty matching
supply without fallback. Conventional observer firing is covered below.

### Direct spotter coordination

Self declaration, acquired friendly observer selection, clearing, saved identity,
status, Lua/native controls and firing restrictions are implemented. Self spotting
requires recycled weapons and limbs. A current-target query validates role, team,
map, power, consciousness and observer contact without requiring firer contact to
the target. Guards protect both complete shots and weapon expenditure. Tests cover
recycling rejection, no-expenditure failures, persistence, role/team/contact loss,
retained established links and native/Lua rollback. Rockets retain indirect
capability without acquiring hotload or semi-guided modes.
Conventional indirect-shot routing and observer aim are implemented below. Artillery data links and hex spotting remain pending.

### Conventional indirect unit fire

With an indirect-capable launcher, a selected observer, and no firer target lock,
`fire` uses the observer's acquired unit target, including when an explicit target
was supplied. The observer provides sensor aim, movement, a +2 settling-lock
penalty and Gunnery-Spotting minus four (default skill target eight without active
crew), plus one for coordination. Firer range, movement, heat, equipment, target
movement and ammunition remain part of the ordinary shot. Firer contact and weapon
arc are not required. Live observer validation precedes any expenditure; previews
clone the firing unit's dice and committed shots use that same stream for sensor
randomness. Indirect shots from above water into submerged targets are rejected.
Native/Lua target routing and rollback are covered. Explicit hex/artillery fire
remains pending.

### Indirect crew experience

Character-capable indirect shots award one Gunnery-Spotting XP to the observer
and one Gunnery-Artillery XP to the firer before damage, independently of hit or
miss. Each recipient needs a connected, present pilot, an in-character unit and a
live hostile in-character target. Spotting uses the ordinary award interval;
artillery is a continuous-XP skill and bypasses it. An earned spotting level
affects the same shot’s final spotting modifier, without repeating sensor dice.
Accepted awards stage MechXP/MechAttackXP diagnostics in the same shot transaction;
misses publish them even without a salvo. Rejected and aborted actions restore
both skill state and output. Simulator units do not gain these awards.

### Indirect feedback and terrain visibility

Indirect firer feedback names the destination coordinates without naming the
hidden unit or announcing a hit/miss. Nearby running observers receive hex-based
launch messages when they can see the firer or terrain coordinate; terrain
visibility needs no acquired unit contact. The target also receives identified
incoming-fire feedback or an anonymous bearing. Audiences and coordinates are
captured before damage, and native/Lua messages share the shot transaction.
`battle_hex_visible` reuses terrain traces at the observer's posture/flight height
and terrain endpoint height. Visual, light-amplification, infrared and EM obey
map/sensor conditions; radar, seismic and active probes require a unit signature.
Tests cover contact-free terrain observation, blocked terrain, signature-only
sensors, native/Lua messages and callback rollback. This is a visibility primitive,
not an implementation of explicit hex fire or artillery.

### Coordinate target selection

`lock x y` selects a unit-at-hex coordinate; H, B, I and C select hex, building,
ignition and clearing purposes. Lua provides `btech.unit.lock_hex`. Selection
checks authority, running state and map bounds, but deliberately requires neither
visibility nor matching terrain. All modes settle after eight committed seconds
and announce the coordinate even when unseen. A single typed target selection
owns either a unit lock or a hex lock; switching replaces it, and `lock -`,
shutdown, placement and sensor resets clear it. Saved data rejects mixed target
kinds, invalid coordinates and countdowns. Native/Lua mode parity, rollback,
mid-countdown persistence and lifecycle clearing are covered. Empty-hex firing,
terrain damage and artillery remain pending; unsupported coordinate attacks reject
before expenditure.

### Unit-at-hex firing

Argument-free target selection in `fire` and Lua resolves a UnitAtHex lock against
the current occupants at attack time. Selection uses persisted map-slot order and
excludes the firer; it does not skip friendly or destroyed units in search of a
preferred opponent. Ordinary visibility, arc, safety, equipment and damage rules
then apply to the chosen occupant. No occupant produces an explicit unsupported
empty-hex result without expenditure. Explicit unit targets override coordinate
locks. Reports carry `coordinate` for coordinate-directed fire and `target` for
the actual damage recipient, sharing the hex broadcast path with observer fire.
The coordinate lock remains selected after the shot; movement does not make it
follow an earlier occupant. Its bearing supplies the ordinary +1/+2 unstable-lock
contribution even after settling, since the selected lock is not a unit identity.
Indirect shots instead use zero firer lock penalty and retain the observer's
settling contribution. Tests cover routing, safety, retained locks, movement,
messages, native/Lua rollback and saved results. Blind missile fire without a spotter remains separate parity work.

### Battlefield membership slots

Placed units own a persisted `map_slot`, distinct from their object IDs and
coordinates. Entry takes the first vacant slot; same-map placement retains it,
while removal, map transfer and destroyed-map cleanup release it. Destroyed units
retain membership until removal. Validation rejects missing, orphaned or duplicate
slots, with uniqueness scoped to the map. `battle_map_unit_order` exposes membership
order and unit-at-hex selection consumes it. Tests insert units in reverse object
order, move and remove them, reuse holes, transfer maps and reload saved slots;
existing placement rollback and map cleanup tests cover the shared lifecycle.
Other map iteration consumers still need individual ordering audits.

### Direct empty-terrain aim

`battle_hex_aim_modifiers` and the pilot-skill variant calculate conventional
empty-terrain aim without a fabricated unit target. Unit and terrain attacks share
weapon range, fire/ammunition mode, movement, heat, damage and equipment accuracy
terms. Unit-at-hex mode gets no hex bonus; H/B/I/C modes get -4. Empty terrain has
no occupant movement, unstable unit lock, target signature, or sensor aim term.
Visibility is reported independently of numeric aim; out-of-range weapons have no
subtotal. Geometry uses terrain height even if an airborne unit occupies that hex.
Lua `btech.unit.aim_hex(unit, weapon, x, y)` returns detached modifiers and subtotal
under the same configured rules as firing. Tests cover modes, neutral target
terms, visibility loss, range, terrain height and read-only Lua inspection. This
preview does not authorize a shot or implement empty-hex expenditure/effects.

### Shared weapon launch resolution

`weapon_launch` owns the candidate-world attack roll, effective mode fallback,
gatling draw, caseless second roll, jams, loader destruction, Streak launch gate,
ammunition/heat/recycle expenditure, glancing threshold, immediate misload damage
and ammunition warning. Unit shots use that pipeline before target defenses and
damage. Heavy Gauss recoil uses a separate shared operation after impact, retaining
its current-condition checks, piloting XP and fall ordering. Both operations run
inside the caller's unpublished world candidate so rejected enclosing actions
restore all dice, inventory and damage. This supplies the common launch machinery
for upcoming empty-terrain attacks; their admission, terrain effects and adapters
remain pending. Existing weapon, character-impact and native/Lua replay suites
exercise the shared path.

### Woodland weapon effects

The detached `resolve_woodland_effect` domain operation resolves deliberate
ignition, deliberate clearing and incidental woodland effects on candidate dice.
Weapon capabilities distinguish fire-starting from clearing (including Gauss,
small lasers, small autocannons and SRM-2 exclusions). Energy ignition uses 5+;
flamers use 4+, ordinary ballistic/missile weapons 9+, and ballistic flechette 5+.
Vehicle flamers retain their ordinary ballistic threshold. Incendiary autocannon
ammunition is distinct from missile inferno ammunition.
Clearing checks accidental ignition first (5 or less deliberate, 3 or less
incidental); that branch preempts clearing even when ignition fails. Heavy woods
become light woods; light woods become rough or grassland. Ignition returns a
60–180 second fire duration without replacing the underlying tile. Tests cover
weapon distinctions, dice replay, branch precedence, duration and non-woodland
immunity. This operation is not yet wired to firing: persisted fire overlays,
expiration, mine removal, terrain notifications and empty-hex admission remain
required before the command path can apply these effects.

### Live woodland clearing

`apply_woodland_clearing` applies a detached clearing result atomically to an
occupied map. It checks the expected tile, permits exactly one woodland reduction,
preserves elevation and occupant state, and leaves checkpoints unchanged.
Persistence accepts woodland reductions alongside ice/bridge breaks, including
two reductions before a save, while still rejecting occupied-map asset reloads.
Integration tests cover rejection without mutation, stale results, both reductions,
occupied-map save/load and batched changes. The caller still owns attack admission,
notices and mine consequences. Temporary fire requires the separate spread/smoke
lifecycle; it must not be implemented as a bare tile replacement with a timer.

### Persistent map decorations

Maps own sparse fire/smoke markers separately from their base terrain dictionary.
`hex` exposes the visible marker with the tile elevation; `base_hex` exposes the
underlying tile, and `decoration` inspects kind and remaining lifetime.
`set_map_decoration` installs, replaces or removes a marker atomically; removing a
marker reveals the base tile. Validation rejects undecoded maps, out-of-bounds
positions and zero lifetimes. Selective SQL persistence preserves unrelated row
columns, rejects corrupt/orphan records, and purges markers with map ownership.
Tests cover occupied maps, checkpoint isolation, replacement/removal, save/load,
invalid requests, SQL failure rollback and corruption rejection. No firing command
creates these markers yet: countdown, fire spread, smoke creation/dissipation,
burnout, notifications and the base/visible-terrain consumer audit remain pending.

### Committed smoke expiration

`advance_map_smoke` decrements smoke markers once per simulation second and removes
them silently at zero, revealing the underlying terrain. Fire markers retain their
lifetime until the separate spread/burnout lifecycle is connected. The server's
activity check includes smoke, so an empty battlefield still advances; expiration
runs before terrain-dependent movement and sensing in the same world transaction.
Tests cover exact expiration, checkpoint isolation, saved countdown replay,
unchanged fire markers, repeated idle steps, and a live server on a unit-free map.
The server test blocks both marker updates and deletion, verifies failed ticks do
not advance persisted state, then restores writes and observes normal expiration.

### Wind and autonomous fire randomness

Maps now read and write their existing wind direction/speed columns, preserving
wind through terrain reloads. `set_map_wind` validates bearing and nonnegative
strength without removing occupants; `fire_spread_interval` implements the
60-minus-strength cadence with a 20-second minimum. Decoded map creation establishes an independent map-owned dice stream, saved
separately from unit dice before the first attack.
Removing fires or reloading terrain retains that stream. Saved fires without a
stream are rejected rather than silently reseeded. Tests cover wind boundaries,
cadence, save/load, retained randomness across reload/reignition and missing-stream
rejection. These are inputs for the pending fire scheduler; fire countdown and
spread are not yet advanced by the server.

### Fire spreading and burnout

Fire now advances in the committed server tick, including on empty battlefields.
Markers persist their next spread delay independently of lifetime; changing wind
retains an already scheduled check and uses the new interval on rescheduling.
Each event checks three wind-relative cells and a second forward cell, with
9/11/11/12 attack thresholds and a wind-strength roll. Smoke is created first on
undecorated non-building/non-wall cells for 90–150 seconds; successful spread then
replaces woodland markers with fires lasting 60–180 seconds. New markers do not
lose a second in the event that creates them. Replacement cancels pending work for
the replaced marker within the step. Burnout removes fire and turns woodland into
grassland on 1–2 of a d6, otherwise rough terrain, preserving elevation.
Map-owned dice and timers save atomically with terrain. Tests cover calm-weather
smoke, wind-driven ignition, geometry/parity/bounds, delayed checks after wind
changes, restart replay and live-server rollback when saving randomness fails.
Simultaneous events use stable map/tile order; cross-fire ordering relative to the
reference event queue remains an audit item. Firing-command admission, target
terrain effects, and base-versus-visible terrain consumer auditing remain pending.

### Physical terrain beneath overlays

Terrain-dependent altitude, range, radar clearance, water cooling, flooding,
movement, jumping, falls, surface fracture and firing/physical-attack guards now
use base tiles. LOS separates base surface heights and water interfaces from
visible smoke/fire attenuation; optical underwater checks use base terrain too.
Fire still contributes heat through the visible marker. Tree-club acquisition and
woodland clearing retain their visible-terrain checks. Tests compare smoked water
with an otherwise identical clear map for altitude, range, underwater LOS, cooling,
flooding and water-entry movement, and fracture ice/bridges beneath smoke through
save/load. Overlay removal reveals any changed underlying terrain. These checks
close the core physical-surface regression introduced by visible map markers;
further parity audits of individual sensors and terrain interactions remain part
of the overall integration.


### Transactional woodland impacts

`resolve_woodland_attack` connects the detached woodland rules to live map changes.
It owns shooter terrain-effect dice, clearing/ignition, and cockpit/observer notices
inside a single candidate world. It does not expend weapons or authorize firing;
launch checks, mine removal and structural effects remain the enclosing shot's
responsibility. Notices distinguish intentional and incidental effects and use
pre-impact observer visibility. New decoded maps establish fire randomness before
attacks, so native/replayed ignition and subsequent spread use the same saved
stream. Existing streams survive asset reloads. Tests cover ignition, deliberate
and incidental clearing, unchanged weapon inventory, notices, restart/spread
replay, invalid coordinates, and rollback after a post-roll application failure.
The direct hex-shot resolver and native/Lua firing adapters call this path.

### Shared weapon damage packets

`weapon_groups` now sizes successful-hit packets independently of a unit target.
The existing unit salvo path supplies target guidance/beacon facts and retains its
candidate target dice; location rolls, AMS reduction and per-packet damage remain
in the salvo resolver. The shared operation owns ordinary/hotload cluster draws,
gatling damage, confused Streak conversion, rotary/double-shot packets, glancing,
range-dependent damage and ammunition guidance. This prepares empty-hex shots to
use the same packet rules without a synthetic defender or duplicated mode logic.
Terrain-specific hit/miss dispatch is integrated in the hex-shot resolver;
existing catalogue, ammunition, salvo and shot suites verify unit-shot behavior.

### Direct empty-hex shot resolver

`resolve_battle_hex_shot` now joins coordinate aim, shared launch/expenditure,
packet sizing, woodland effects and Heavy Gauss recoil in one candidate world,
returning `BattleHexShotReport` without a synthetic defender. It checks control,
readiness, direct visibility, mounting arcs and submerged-fire restrictions before
consuming dice. Unit-at-hex occupants route through the separate unit resolver;
H/I/C terrain modes do not damage occupants. Successful terrain shots apply damage
packets to woodland; non-missile misses can still trigger the appropriate woodland
check, while missile misses do not roll hit packets. Flechette keeps full terrain
damage rather than applying its armored-target reduction. Terrain packets do not
receive the unit-only glancing damage reduction. Launch failures and rejected
attacks retain the shared rollback behavior.
Tests cover energy hits/misses, ignition/clearing, inventory/recycle, untouched unit
occupants, saved replay, missile hit/miss expenditure and admission rejection.
This public resolver currently handles direct non-character shots. Building and
blind/indirect hex fire and mine effects remain pending.
Native and Lua firing now dispatch coordinate selections through a character-aware
host adapter, sharing launch failure feedback and configured rules with unit shots.
Its checkpoint includes terrain, expenditure, recoil/misload injuries, experience,
casualty evacuation and staged output. Tests cover native/Lua equivalence for
ignition, clearing, empty-coordinate shots and Heavy Gauss recoil, with character
crews, aborted callbacks and saved-state replay.


### Weapon-triggered ice and bridge breaks

H-mode successful packets now apply nominal-damage structural checks after woodland
checks, using persisted shooter dice. Ice uses 15 faces; bridges use ten times one
plus elevation and honor the bridge-capacity flag. Reports preserve failed rolls,
visible bridge shudders and successful fracture/occupant consequences. Native/Lua
publication includes character falls and casualty handling in the shot checkpoint.
Tests exercise success/failure, capacity immunity, occupant falls, deterministic
replay, persistence and native/Lua character callback rollback.


### Interior-map building state

`StoredBattleMap.building` owns current/maximum construction integrity, building
policy flags and the regeneration factor. The persisted `cf`, `cf_max`,
`build_flag` and `regen_factor` columns now load and save through this checked Rust
state. `set_building_state` updates an interior without replacing terrain or moving
occupants; the caller owns administrative authorization. Asset reloads preserve
construction state. Integrity is bounded by its nonnegative signed-short maximum;
flags retain one byte and regeneration retains its signed integer range.

Policy queries distinguish command centers, complex interiors, hidden/dropship
structures, invisibility and safety. Safety is not treated as damage immunity.
Tests cover occupied updates, invalid values, save/reload, zero integrity,
transactional database failure, terrain reload and corrupt saved integrity.
Entrance references, building weapon damage and repair scheduling are described
below; configuration alone does not schedule repairs.


### Building entrance references

`BattleBuildingEntrance` links a battlefield coordinate to the interior map that
owns construction integrity. `set_building_entrance` configures or removes a stable
ordinal; `StoredBattleMap::building_at` selects the first matching ordinal and
`building_entrances` exposes the ordered entries. Duplicate coordinates and multiple
entrances to one interior are supported. Selection does not filter hidden, safe,
or command-center policies; those belong to the consuming action.

Type-four rows in `btech_map_objects` persist these references. Updates own only
coordinates and the interior reference, preserving auxiliary entrance data and
other object kinds. Invalid coordinates and missing interior maps are rejected.
Terrain reloads retain entrance references, and map purges remove incoming links.
Tests cover ordering, lookup, invalid updates, restart, auxiliary-data preservation,
unrelated object preservation, failed deletion rollback and corrupt references.
Building firing effects and regeneration use these references. Movement through
entrances remains separate integration work.


### Building firing and repair

Native and Lua fire now resolve B-mode successful packets against the first building
entrance. Damage is capped at the interior's remaining integrity; absent and already
ruined buildings are quiet. Complex source maps and command-center targets emit the
paint-scratch response. Packets report `buildings` with interior identity, actual
damage, remaining integrity and captured notices. Shooter, interior contents and
visible destruction observers receive feedback within the firing checkpoint.

The first surviving hit from full integrity schedules repair in 120 committed
seconds. Further damage does not reset that clock. Each event adds the configured
factor, caps integrity at its maximum and reschedules while damaged. Signed factors
use bounded nonnegative integrity arithmetic. Countdown and integrity save together,
including on otherwise idle maps; save failures restore both. Restart resumes the
saved countdown without offline catch-up. Destruction does not schedule rebuilding:
the reference's zero-integrity guard prevents its rebuild branch from being reached.
An already pending repair event stops when it encounters a destroyed building.

Tests cover partial/lethal damage, immunity, absent buildings, misses, continuing
repair clocks, restart replay, full repair, native/Lua character rollback and interior
messages. An idle-server test forces a failed repair save and checks successful retry.
Entrance movement and explicit repair/rebuild administration remain pending.


### Typed minefield definitions

`BattleMinefield` stores coordinate, kind, signed strength, extra setting and owner.
`BattleMineKind` distinguishes standard, inferno, command, vibra and scripted trigger
fields. Stable ordinals preserve ordering and permit multiple fields at one hex.
`set_minefield` performs checked administrative configuration without changing
terrain, occupants or dice; `StoredBattleMap::minefields` exposes the saved records.

Type-three map-object rows persist all definition fields. Updates preserve extension
columns and unrelated map objects. Invalid kinds, bounds and owners are rejected;
map/owner purges remove the corresponding definitions. Asset reloads retain fields.
Tests cover all kinds, ordering, invalid updates, restart, extension preservation,
failed deletion rollback and corrupt kinds.

Weapon clearing deliberately leaves mines intact: the reference's
`mine_field_possibly_remove` performs no removal. A successful woodland-clearing
test verifies this with all five mine kinds. This closes that clearing-parity gap;
Physical activation and trigger callbacks are described below; radio command
detonation remains pending.


### Mine coverage and activation selection

`StoredBattleMap::mine_coverage` derives the coverage union from saved definitions,
without a second mutable bitmap. Standard/inferno/command fields cover their own
hex. Vibra fields use the extra-setting coverage diamond, while trigger fields use
the rounded continuous radius within its coordinate bounds. Negative and zero
settings retain their distinct coverage behavior.

`mine_activations(world, unit, reason)` selects ordered pre-event definitions using
current whole-ton mass and base-terrain altitude. Ice uses surface level zero even
under smoke. Command mines at the unit coordinate return `Spotted`; ordinary fields
return `Explode`. Vibra fields distinguish colocated weight eligibility from remote
weight/range checks. Trigger fields return `Trigger` only for step/land reasons and
sufficient weight. Coverage gates the map-wide scan, so one field can admit a
colocated field that has no coverage of its own.

These are read-only queries: no dice, damage, deletion, notices or callbacks occur.
Tests cover geometry, thresholds, reason exclusions, stable order, smoke/ice,
above-surface rejection, invalid queries and restart equality. The selector is ready
for the physical activation transaction described below; radio detonation remains pending.


### Conventional mine blasts

`resolve_mine_blast` resolves an admitted standard, command or vibra field inside a
candidate world. Each affected biped receives kick-table damage in five-point
packets, with shared immediate critical, balance and flooding consequences. Map-slot
order selects occupants; center damage precedes coordinate-ordered adjacent hexes.
Command/vibra neighbors receive half damage and can ignite undecorated woodland
using the map's saved fire stream. Blast altitude is limited to ground zero and the
level immediately above it; water and ice use signed bottom depth.

Standard fields below strength five are removed. Stronger standard fields retain
their strength; the reference's integer depletion multiplier never updates them.
Command and vibra blasts remove fields at the detonated coordinate, including other
mine definitions there. Reports retain hits, ignition, removed ordinals and notices.
`resolve_mine_blast_action` adds character injuries, casualty evacuation and output
rollback under the host checkpoint. Trigger admission/feedback belongs to the caller.

Tests cover damage packets, neighboring targets/fire, removal, saved replay, late
post-damage failure, mixed character occupants and water-height boundaries. Activation
queries now correctly use signed water depth too. Negative-strength blast behavior,
radio detonation and remaining mine administration remain pending. Inferno
blasts use the burn lifecycle described below; scripted blasts reject before mutation.


### Inferno burns and mine damage

`apply_inferno_burn` extends a bounded, saved duration without an immediate heat
increase. While burning, six points are subtracted from dissipation after water
cooling, floored at zero before environmental temperature adjustments. Committed
simulation ticks age the duration after sampling heat, including in shut-down units.
Expiry publishes the cockpit cooling message once. Failed saves restore the timer
and feedback; restart resumes the saved duration without offline catch-up.

Inferno mines damage occupants at their own coordinate for one third of strength,
in five-point kick-table packets, then apply six burn seconds per strength point.
Strengths one and two still burn despite having no armor damage. Weak fields below
five are removed; stronger fields persist. Damage, burn extension, field removal
and character consequences share the blast checkpoint. Duration overflow rejects
the entire blast, including any earlier occupant damage.

Water entry, landing and falling extinguish burns on water terrain, except for a
standing unit in depth-one water. Extinction cancels the clock, publishes steam
feedback and replaces the hex decoration with 120 seconds of smoke. Burning units
illuminate their own and adjacent hexes. Status heat/info views and Lua inspection
expose remaining burn seconds.

Tests cover cooling, extension, duration bounds, illumination, mine damage/removal,
late failure rollback, water depth/posture, steam, saved replay and a shut-down
server's persistence-failure retry. Negative-strength mine effects and radio detonation remain unfinished.


### Inferno missile exposure

`resolve_inferno_hit` applies an admitted, positive missile count after cluster
selection and interception. Each pair of surviving missiles, rounded up, adds
180 seconds of burning. Repeat exposure extends the clock with the brighter-fire
feedback; first exposure publishes ignition feedback. This operation neither
consumes dice nor changes armor, health or stored heat.

Ignition is followed immediately by the shared water-extinguishing check, so a
submerged target receives ignition and steam feedback while retaining no burn.
Observer messages use acquired, currently visible contacts. A candidate world
contains duration changes and steam decoration together; overflow, invalid targets
and late validation failure leave the caller's world untouched. The detached
`BattleInfernoHit` report retains target, missile count, added duration, extinction
and ordered notices for publication by the enclosing action.

Tests cover paired-missile rounding through the supported count limit, repeated
hits, unchanged unrelated state/dice, observer feedback, water depth/posture, ice,
restart replay, invalid targets, overflow and late rollback after steam creation.
Firing and ammunition-explosion callers share this operation, as described below. Reference behavior: `combat/mech_combat_misc.c` and
`combat/mech_combat_missile.c`.


### Inferno ammunition controls, firing and explosions

Native `inferno <selection>` and Lua `btech.unit.inferno` toggle the shared
ammunition selection, with ordinary pilot/readiness guards and one-shot rejection.
Template `Inferno` flags select typed bins and mounts; capacities, half-ton mass,
matching-bin expenditure and persistence use the existing ammunition model.
Weapon listings show `[Inferno]`, and compact exports use `I`. As in the reference's
actual mode predicate, missile compatibility includes long-range launchers;
Narc/iNarc pod behavior retains its existing priority over burn exposure.

Unit firing computes clusters and AMS interception before applying the shared
inferno hit. Inferno salvos have no armor packets or damage XP; the optional
`salvo.inferno` report contains exposure and feedback. Misses and complete
interception do not burn. Streak lock failure retains its ordinary launch behavior.
Late burn failure rolls back ammunition, recovery, heat, dice and defense effects.
Native and Lua character actions publish the same saved state and notices.

A successful inferno terrain shot invokes one zero-damage terrain exposure rather
than one per missile. It can ignite woods at five or better, including small SRMs,
but cannot clear woods or damage buildings. Ordinary H-mode ice/bridge checks still
occur once. Unit-at-hex targeting retains its existing occupied/empty routing.

Inferno bin explosions apply exposure from one quarter of their nominal potential,
then halve internal damage. With `battletech.inferno_penalty`, they also add thirty
stored heat and use earlier thermal explosion checks: targets 4/6/8/10/12 at heat
10/14/19/23/28. Available inferno bins take priority over larger ordinary bins;
empty or unavailable bins do not activate the penalty. Explosion injury and
character consequences remain in the existing damage transaction.

Tests cover template capacities and mass, control/fire parity and callback rollback,
restart, partial/complete interception, duration overflow, single terrain exposure,
bin damage and water extinction, configurable heat and all thermal thresholds.
Indirect hex fire, other vehicle classes and additional missile families remain
part of their unfinished feature gates.


### Physical mine activation and scripted triggers

Ground hex entry, successful jump landing and completed falls now activate saved
minefields through `activate_mines` and its character-capable transaction. Flat
hex crossings participate even when there is no height, water or ice check. Steps
emit coordinate-specific trigger feedback; falls and landings use the shorter
trigger message. Local command fields publish bomblet spotting without detonation.
Remote vibra detonations announce their visible hex and apply neighboring damage.

Activation snapshots retain ordinal order. Fields deleted by earlier blasts are
skipped. Blast-induced support falls retain their own activation reports, including
further damage to occupants. Movement stops at a triggering step when the unit
falls, shuts down or is destroyed. All movement, dice, damage, burns, field deletion
and queued feedback remain inside the enclosing world checkpoint.

Each selected scripted field queues `on_mech_mine_trigger` on the triggering unit's
attached Lua object module. Its event context uses that unit for object, enactor
and cause, with operation `mine_trigger` and no descriptor. Movement and landing
host actions publish these callbacks together with injury consequences. Callback
failure restores movement, damage, persistent script state and output. Detached
low-level movement functions return notices; the host actions own Lua publication.

`BattleMineEventReport` retains the triggering unit/reason, blasts, callback count
and notices. Fall reports retain their mine event; movement reports retain entry
and landing events. Existing casualty handling visits nested mine injuries and
falls without duplicating their public notices. No new persistence format or C
bridge is required.

Tests cover flat entry, command spotting, inferno burns, restart replay, landing,
fall activation, nested support loss, deletion of later selected fields, remote
vibra feedback, late blast failure and character callback rollback. Radio command
detonation, negative-strength blasts and remaining map administration are still
unfinished.


### Frequency-matched command mine detonation

`detonate_command_mines` resolves the mine phase of an admitted transmission.
It snapshots matching command fields on the sender's map in ordinal order, without
owner, distance or altitude restrictions. The supplied frequency is matched exactly
against each field's signed extra value. Transmission controls own channel limits,
radio readiness and message admission. This operation does not send radio messages.

Each actual detonation captures visible hex feedback, resolves the existing area
blast, then removes colocated definitions. Fields removed by earlier detonations or
nested falls are skipped. Non-command fields and other frequencies/maps remain
untouched except for ordinary colocated deletion and blast consequences. Repeating
a consumed frequency is a no-op with no dice draws or notices.

`BattleCommandMineReport` retains sender, map, frequency, ordered blasts and notices.
`detonate_command_mines_action` publishes character injuries, nested fall effects
and casualties within one host checkpoint. A late blast or publication failure
restores every earlier change and pending notification. Physical activation,
individual blasts and command detonation share the same injury-publication helper.

Tests cover multiple matching fields, deleted later definitions, distant fields,
different owners, map isolation, restart replay, zero/signed frequency values,
invalid senders, no-op repetition, character publication and late failure rollback.
The shared transmission action below encloses this detonation phase with radio delivery. References:
`combat/mine.c` and `ui/mech_notify_radio_config.c`.


### Radio channel configuration

Constructed units save sixteen channel slots, exposing the active subset derived
from template radio quality. Qualities 1–5 provide 2/4/5/8/11 channels and ranges
64/80/100/120/140. Missing or zero quality defaults to 3 for IS and 5 for Clan.
Clan radios and quality 4–5 radios support relay. Unsupported quality is rejected.

`setchannelfreq A=123`, `setchanneltitle A=Command`, `setchannelmode A=DuG` and
`listfreqs` use the shared Rust channel model. Configuration requires a conscious
assigned cockpit pilot and works while shut down. Frequencies are 0–999999;
titles retain up to fifteen bytes without splitting UTF-8 characters. D selects
digital, U mutes, E enables relay with digital/capable hardware. Colors use
`xrgybmcwXRGYBMCW`; the last color wins. Unknown suffixes end mode parsing.
Empty mode resets to analog. Info and scanning modes are unavailable on these radios.

Lua exposes transactional `btech.unit.radio_frequency`, `radio_title` and
`radio_mode`, each taking unit, pilot, zero-based channel and value. `btech.unit.state(unit)`
exposes active settings in `radio` (a one-based array) and derived limits in
`radio_capabilities`. All sixteen saved slots are validated on load. Tests
cover native/Lua state parity, hardware limits, restart, rejected operations and
callback rollback. Changing configuration sends no messages and triggers no mines;
radio delivery and transmission admission use the separate shared action below.


### Digital radio delivery and relays

`resolve_digital_radio(world, sender, channel, message)` computes digital
receptions without mutation, dice draws or publication. It returns the frequency,
map and ordered `BattleRadioReception` records, including the receiving channel,
transmitter path, bearing and formatted cockpit text. `notices()` projects the
result into ordinary cockpit notifications for the enclosing host transaction.

Each unit listens on its first matching unmuted channel, regardless of that
channel's analog/digital selection or the receiver's power state. Destroyed and
pending-removal units do not participate. The transmitter's three-dimensional
range bounds each directed hop. Friendly running units with capable hardware and
a matching relay-enabled channel can forward; muting that channel does not disable
relay. Each unit appears once in the graph, even with multiple matching channels.
Breadth-first search finds the fewest hops without recursive fallback or arbitrary
search cutoffs. Equal-length paths use saved map order deterministically.

Endpoint ECM suppresses remote digital reception, while self-monitoring remains
available. Intermediate relay ECM does not suppress forwarding. Bearings point
toward the last transmitter, colors belong to the receiving channel and titles
belong to the sending channel. Direct paths take priority over relayed paths.

Tests cover multiple hops, equal-length alternatives, directional range boundaries,
shutdown and hostile relays, muted forwarding, first eligible receiving channel,
ECM at each role, formatting and exact restart replay without world mutation.
The shared transmission action below exposes this delivery through `sendchannel`.
Autopilot delivery requires its remaining runtime implementation.
Behavioral reference: `ui/mech_notify_radio.c`.


### Analog radio reception and interference

`resolve_analog_radio` delivers to each same-map unit's first matching unmuted
channel, including frequency zero and powered-down receivers. Receiving channel
mode does not restrict analog reception. There is no hard range cutoff or relay
path for analog: distance beyond sender range causes one interference pass;
distance beyond the larger of receiver range and the average of both ranges
causes a second, weaker pass. Equality at either boundary causes no pass.
Endpoint ECM adds a final pass at distances of at least one hex, excluding self.

The payload includes the sender title before scrambling. Each pass makes
conditional signal and 2d6 communication checks, bounded character displacement,
and printable clamping. ASCII/Latin-1 retain these rules, including space clamping;
larger Unicode characters are retained intact without draws. Receiver color and
channel/bearing framing are applied after interference. Each receiver owns its
saved dice stream, so identical saved state replays exactly. The entire operation
uses a candidate world: a late failure discards all receivers' earlier draws.

Units save `radio_skill`, initially 6, then sample `Comm-Conventional` with the
pilot's mental attributes at startup completion. A player's missing character
profile yields 18; no player pilot yields 6. Later skill changes take effect at
the next startup. Lua's `btech.unit.state` exposes this captured target.

`BattleAnalogRadioReport` includes ordered receptions and `interfered_receivers`
for the enclosing host's XP policy; it does not award XP or publish messages.
Tests cover boundary arithmetic, character/dice order, two-pass range loss,
endpoint ECM, self-monitoring, muting, zero frequency, restart, late rejection
rollback and startup skill sampling. The shared host action applies the communication XP gate described below.


### Native and Lua radio transmission

`sendchannel A=Message` and `btech.unit.radio_send(unit, pilot, 0, message)` use
`send_radio_action`. Both letter cases select the same zero-based channel. The
pilot must be conscious and assigned to that cockpit; crew stun and destroyed
hardware prevent sending. Shutdown does not. Empty text, control characters and
invalid channel indices are rejected before reception or mine effects.

The selected channel chooses analog or digital delivery. Receptions are published
first, then every matching command mine on the sender's map is resolved through
the character-capable mine host action. No receiver or relay is needed for the
mine phase, and frequency zero remains an ordinary exact match. Mine damage uses
the configured hit, stagger, stacking and piloting rules.

One outer checkpoint covers reception dice, all notifications, mine damage and
nested character consequences. A late error restores the pre-transmission world
and outbox. A later error in the enclosing Lua callback also rolls back the whole
action. The returned `BattleRadioTransmission` retains tagged delivery diagnostics
and the command-mine report. Tests verify both modes, native/Lua parity, shutdown
sending, restart, rejected admission and late mine/callback rollback.

Observer scanner privileges and autopilot radio commands remain separate integration work.


### Radio administrative diagnostics

Native and Lua frequency setters use `set_radio_frequency_action`, combining the
setting and `MechFreqs` publication in one checkpoint. A positive frequency emits
one alert for every matching channel on a different-team unit on the same map,
in stable unit/channel order. Muting, shutdown and repeating the same setting do
not suppress alerts. Frequency zero, same-team units and unplaced senders produce
none. This is diagnostic feedback; it does not prohibit sharing a frequency.

A zero-frequency transmission on an in-character map captures a `ZeroFrequencies`
record with player name/id, unit, channel, map and message. The map's flag controls
eligibility independently of the unit's flag. The shared transmission action
publishes that record before cockpit delivery and mine effects, under the same
outer checkpoint. Its returned report includes `audit_messages`.

Both diagnostics use existing ordinary channels and their history, membership and
persistence behavior. Missing channels are ignored rather than created. A channel
publication failure, a later mine failure or an enclosing Lua callback error
restores settings, reception dice, channel counters/history and queued messages.
Tests cover native/Lua parity, duplicate enemy channels, muted/shutdown reception
settings, repeated/zero frequencies, channel overflow, map flags and rollback.


### Communication experience cadence

The transmission host awards one `Comm-Conventional` XP for interfered analog
reception by an in-character unit with a connected player pilot. Accepted awards
publish to `MechXP` and appear in `experience_messages`. Digital and clear analog
reception do not attempt communication XP. The existing skill catalog and ordinary
skill award interval remain authoritative.

Each in-character interfered receiver consumes a saved 61-second gate when due,
even if it has no connected pilot or the skill-level interval declines the award.
A gate still at one second after sixty ticks prevents another attempt. The next
tick permits one. Out-of-character reception does not consume the gate. New units
and completed startups have no pending gate; saved gates survive server restarts,
and offline time does not advance simulation countdowns.

`radio_experience_remaining` is stored with unit state, validated in 0–61 and
exposed in Lua inspection. Shutdown gates keep the simulation tick active and
advance through the ordinary world-save boundary. Award, countdown, diagnostics,
reception dice and subsequent mine consequences share the transmission checkpoint.
Tests verify exact boundaries, shutdown/restart, independent skill throttling,
disconnected pilots, publication/callback rollback and server failed-save retry.


### Observer radio reception

`set_battle_observer` is a trusted scenario edit, with a saved `observer` flag;
radio commands do not let pilots grant themselves this role. Lua inspection
exposes the role and `battlefield_id`. The latter derives from the saved map slot
using base 36 (`AA`, `AB`, … `A9`, `BA`, …), retaining restart identity and slot
reuse without a separate random identity allocator. Labels grow beyond two
characters for large maps. These labels do not yet replace dbrefs in target input
or ordinary contact displays, and are not randomized legacy callsigns.

An observer receives on its first unmuted channel, regardless of tuning. Digital
reception bypasses range, relay-path and endpoint ECM rejection. Analog reception
shows the clear payload while retaining its normal interference draws and XP
eligibility. Both formats identify the source affiliation, battlefield label,
frequency and title, using team color rather than receiving-channel color.
A missing or pending-deletion affiliation renders empty. Fully muted and destroyed
observers receive nothing, and each transmission produces at most one observer
reception. Shutdown does not disable reception.

Tests cover remote/ECM reception, untuned and muted channels, team-color selection,
clear analog text with interference, affiliation removal and exact restart replay.
Observer privileges in scanner/map views remain integration work alongside autopilot radio handling.


### Targeted line-of-sight radio

`radio #123=Message` or `radio AB=Message` and Lua
`btech.unit.radio_target(sender, pilot, target, message)` share a targeted radio
host action. Native labels are case-insensitive and refer to saved battlefield
slot labels; explicit dbrefs match the existing contact-command convention.
The conscious assigned sender must be running, not destroyed and not an observer.
The target must be an acquired contact still visible under current sensor rules.
This operation neither scans nor spends dice, and does not touch channel settings,
communication XP, radio audits or command mines.

The sender receives a confirmation identifying the visible target. A running
target receives a separate message: if it cannot see the sender, the name is
`something`; a visible friendly identity uses lowercase battlefield letters.
Shutdown recipients get no cockpit delivery. Sender and recipient identities are
captured independently, without giving either unit a new contact. All notices use
one output checkpoint, also covered by an enclosing Lua callback transaction.

Tests cover asymmetric identification, native/Lua parity, labels/dbrefs, unchanged
world state, restart, callback rollback, unacquired targets, observer exclusion,
shutdown source/target behavior and invalid text. Autopilot reply handling remains
pending with the autopilot runtime.


### Observer contact notifications

Automatic contact events now project their cockpit notice through
`BattleContactEvent::notice`. Observer mode suppresses ordinary acquisition and
loss chatter while preserving the independent weapon-lock-loss warning. The
server uses this projection through the same staged notification path as other
unit messages. Acquisition rolls, contact state, sensor range, visibility and
lock cancellation are unchanged by observer mode.

Tests compare ordinary and observer scan results and complete saved simulation
state, then force contact loss to verify that only the lock warning remains.
Reloading preserves the notification policy. Observer mode does not automatically
acquire contacts or expand acquisition range. Detailed unit scans are described below;
tactical-map commands remain integration work.


### Detailed unit scans

`scan [target] [A|I|W|AIW]` and `btech.unit.scan(unit, pilot, target, options)`
share `scan_battle_unit`. Native targets accept battlefield labels or dbrefs;
omitting the target uses the selected unit lock. The assigned conscious pilot
must have a running, intact unit, functioning scanner hardware and an acquired,
currently visible target with clear terrain LOS. This read-only query consumes
no dice and does not change contacts or notify the target.

Computer grades one through five provide scan/tactical radii of 16, 20, 25, 30
and 35 hexes, with twice that radius for long-range hardware. Zero or missing
quality defaults to grade three for Inner Sphere and five for Clan. One sensor
critical halves the radii (rounding down); two disable them. Detailed scans compare
the truncated spatial distance with the scan radius. Detached Lua unit state
exposes these hardware values as `sensor_ranges`; they do not replace acquisition
sensor limits.

Ordinary armor reports expose integer condition bands and destroyed sections;
weapon reports expose readiness without ammunition, modes or exact recovery times.
Administrator-assigned observers receive exact status and bypass the distance
check, while retaining contact, LOS and hardware requirements. Tests cover grade
and damage limits, armor band boundaries, disclosure, native/Lua parity, authority,
range/visibility failures, unchanged state and saved replay. Tactical/long-range displays remain open.


### Scan warnings

Native `scan` and Lua `btech.unit.scan` use `scan_battle_unit_action` to stage a
running target's cockpit warning with the enclosing transaction. The target's own
contact visibility determines whether it sees the scanner's name or `something`;
friendly identified scanners use lowercase battlefield labels. Observer scans
and shutdown targets produce no warning. The raw `scan_battle_unit` query remains
read-only, allowing inspection without publication. Failed scans and aborted Lua
callbacks leave no staged warning or changed simulation state. Tests cover both
identity views, native/Lua output, observer/shutdown silence and rollback.
Autopilot scan replies remain pending with the autopilot runtime.


### Coordinate unit scans

Native `scan x y` and Lua `btech.unit.scan_hex(unit, pilot, x, y, options)`
inspect the first acquired visible occupant at that coordinate, using saved map
membership order. They exclude the scanner itself and skip unacquired occupants.
An empty hex and a hex containing only unacquired units both return
`You see nobody in the hex!`; scanning never acquires or selects a target.
Coordinates require map bounds, scanner distance and terrain sensor visibility;
the selected occupant then passes detailed unit-scan guards and shares its report
and transactional target-warning path. Observer distance exemptions retain
visibility and hardware requirements.

Tests cover stacked occupants, selection after contact loss, empty/unacquired
privacy, native/Lua parity, saved replay, map bounds, authority, range and visibility.



### Structure scans

`scan x y B` and `btech.unit.scan_building(unit, pilot, x, y)` inspect the first
entrance at a coordinate and read its interior map's current construction factor.
The report is delivered to cockpit occupants. Bounds, visibility, pilot/power and
hardware guards share coordinate scan admission. Explicit structure coordinates
retain the hardware distance limit even for observer units.

Invisible, missing and unavailable interiors give the same no-building reply
without dice or XP. Hidden structures require an in-character scanner with a
connected player pilot. They roll two persisted unit dice against startup-captured
perception plus the rounded coordinate/altitude distance. Failed rolls reveal
nothing. Successful rolls reveal integrity and attempt one Perception XP, subject
to the shared skill interval, with accepted awards reported to MechXP.

`scan_battle_building` accepts an explicit timestamp and commits the domain result
atomically; `scan_battle_building_action` also stages cockpit/channel delivery and
restores the entire state on failure. Tests cover native/Lua output, concealment,
authority, observer distance limits, exact dice use, XP intervals, saved replay and
Lua callback rollback.


### Mine recognition and full hex scans

Native `scan x y H` and Lua `btech.unit.scan_terrain(unit, pilot, x, y)` inspect
buildings first, then mines, within one world/output checkpoint. Structure output
reaches the cockpit. Failed mine recognition is private to the pilot; success
reaches cockpit occupants. Reports disclose recognized mine presence only, never
kind, strength, frequency, owner or trigger settings, and never detonate a field.

Mine recognition requires a field authored at the scanned coordinate. Empty hexes
consume no dice. Occupied coordinates draw an inclusive 2..9 range gate, compared
with truncated spatial distance, before testing in-character perception eligibility.
A passing gate permits a two-die check against cached perception with no distance
modifier. Successful recognition attempts one Perception XP through the same
interval and diagnostic path as hidden-building scans. Both phases share one
wall-clock timestamp; an accepted building award can suppress the mine XP award
without suppressing detection.

`scan_battle_mines` accepts an explicit timestamp and commits domain state atomically;
`scan_battle_hex_action` includes both scans and all output. Tests cover dice order,
empty/OOC/far outcomes, recognition, private versus cockpit delivery, native/Lua
parity, unchanged fields, saved replay and callback rollback of both phases.



### Scanning saved targets

A plain `scan`, or a report option such as `scan A`, now uses the complete saved
target selection. Unit locks inspect that unit; building locks inspect the
structure; full-hex locks inspect buildings then mines. Unit-at-hex, ignition and
clearing locks inspect the first acquired visible occupant. A scan works while
the target is still settling and never replaces the lock or advances its countdown.
Missing selections produce `No default target set!`.

`scan_battle_selected_action` and Lua `btech.unit.scan_selected(unit, pilot, options)`
share dispatch and return a tagged unit/building/hex report. Coordinate selections
recheck clear terrain LOS and retain the appropriate scan admission checks.
Observer units may bypass hardware distance for selected coordinates; explicit
`scan x y B/H` keeps its range limit. Observer status does not bypass visibility,
disabled hardware, or perception eligibility. Selected structure reports publish
through their existing transactional actions, including rollback of dice and XP.
Tests cover all five coordinate modes, native/Lua report and output parity,
unchanged countdowns, restart, distance/visibility checks and callback rollback.


### Brief unit reports

`report [target]` and `btech.unit.report(unit, pilot, target)` share the read-only
`report_battle_unit` summary used at the top of detailed scans. Reports show identity,
range/bearing, speed/heading, coordinates, heat and unit type without detailed armor,
weapons or ammunition. They require the assigned conscious pilot, a running unit,
working scanners and an acquired visible target with clear terrain LOS. Direct
reports do not enforce detailed scan distance and never warn the target or spend dice.

Native `report x y` selects the first acquired visible occupant in saved map order,
subject to coordinate range/visibility limits. A plain `report` uses the selected
unit or the occupant of the actual saved hex coordinate without changing the lock.
Tests cover native/Lua parity, disclosure, silence, direct versus coordinate range,
empty/unacquired occupants, saved selections, authority, visibility and restart.

The reference scan/report handlers accept map coordinates, not a separate projected
bearing/distance scan form. Earlier projected-scan notes have been removed; navigation
and tactical-map projection remain separate coverage areas.


### Display center resolution

`resolve_battle_view_center` and `parse_battle_view_center` share tactical and
long-range display centering. Requests select the own unit, an acquired visible
contact, or an integer compass bearing and signed distance from current continuous
position. Negative distances reverse the compass. Ordinary projection limits use
the absolute truncated distance; observer projections bypass that limit. Contact
centering always retains spatial range and current contact visibility checks,
even for observers. All modes require a conscious assigned pilot, running unit
and working display hardware.

Lua `btech.unit.view_center(unit, pilot, kind, arguments)` returns the map, center
and damage-adjusted hardware radius. It changes no simulation state, target lock,
contacts or dice. Projected centers may lie outside the map; a renderer must clip
its viewport and apply terrain/contact visibility separately. Extreme or nonfinite
projections fail within the shared geometry bounds. Tests cover compass wrapping,
signed/fractional range boundaries, labels/dbrefs, observer policy, hardware loss,
visibility, authority, detached Lua parity and saved replay.

This supplies display center resolution only. Tactical and long-range rendering,
overlays and player display preferences remain unfinished; viewport sizing is described below.


### Bounded display viewports

`resolve_battle_viewport` and Lua `btech.unit.viewport(unit, pilot, kind, arguments,
dimensions)` resolve an in-bounds rectangle from the shared display center.
Requested tactical dimensions default to 21 by 14 and validate within 5..40 by
5..24. Both axes then cap at twice the damaged tactical radius and the map size.
Long-range height defaults to 11, validates within 10..40, caps to hardware/map
height, and rounds up to an odd height before final map clipping. Its horizontal
span includes 71 cells, matching the reference's inclusive 70-column span.

The returned origin, width and height use count semantics and stay inside even
one-cell maps. Extreme projected centers clamp to the nearest usable edge without
overflow. Requested dimensions are supplied per call; persistent player preferences
remain separate work. Tests cover boundaries, small maps, damage, odd heights,
projection clipping, invalid dimensions, Lua parity, unchanged state and restart.
This is renderer input only; terrain/contact disclosure and text rendering remain
unfinished.


### Long-range terrain, elevation and unit maps

Native `lrsmap T|E|M [target|bearing distance]` and Lua
`btech.unit.lrsmap(unit, pilot, mode, arguments)` now render bounded long-range
maps using shared centering and viewport rules. T shows effective terrain,
including fire/smoke overlays; E shows numeric elevation/depth with blank zeroes;
M overlays currently acquired visible units on terrain. The scanner uses `*`,
friendly bipeds `b` and hostile bipeds `B`. Stacked contacts use saved map order,
with the scanner's own marker taking precedence. Other unit classes remain gated
by construction support.

Rows stagger odd and even columns across two text lines with column and row labels.
Dark maps mask evaluated hexes failing current terrain visibility with `?`, while acquired
visible contact markers retain their independent unit visibility. Ordinary maps
show terrain independently of contact acquisition. Rendering changes no contacts,
locks, dice, timers or output. Tests cover native/Lua parity, exact row positions,
water depth/zero elevation, map edges, overlays, friendly/enemy markers, missing
contacts, dark-map masking, observer projections, authority and saved replay.

The finer seen/terrain/elevation LOS masks remain unfinished. Dark-map rendering currently uses the shared boolean hex
visibility query. Tactical rendering and saved display preferences also remain
integration work.


### Long-range map styles

The shared map palette now supplies terrain, own-unit and relative-contact styles.
ANSI-enabled pilots receive colored T/M displays: water depth distinguishes bright
from ordinary blue, woods distinguish bright from ordinary green, fire is bright
red, own-unit markers are bold, friends bright yellow and enemies bright red.
Plain T/M output keeps identical visible glyphs and spacing. E remains uncolored
and uses blank zero elevation; C (`colored_elevation` in Lua) applies terrain colors
to numeric elevation and displays zero explicitly. Client capability handling still
owns final ANSI/HTML output.

Cell data is escaped independently from trusted palette markup. Adjacent styles
are coalesced, and both staggered rows reset before coordinate labels or the next
line. Evaluated but obscured dark-map cells use blue question marks. Tests compare styled/plain display
geometry, own/friendly markers, fire and zero elevation, label reset boundaries,
ANSI preferences, native/Lua parity and saved replay. Configurable palettes are
not introduced; the reference's default map palette is shared for future renderers.


### Explicit long-range visibility modes

`lrsmap L`, `H` and `S` apply the current terrain visibility query on any map:
L displays visible terrain, H visible elevation, and S visible units over filtered
terrain. Lua accepts those letters or `visible_terrain`, `visible_elevation` and
`visible_units`. H retains uncolored elevation and blank zeroes; L/S honor the
pilot's ANSI preference for visible cells. Acquired unit markers remain governed
by their independent contact visibility, with the own-unit marker available in S.

Every requested cell is evaluated. An obscured result therefore uses the reference's
blue `?` marker, replacing the earlier red `X` used for these cells. X denotes an
unevaluated cell in the reference; this renderer evaluates its complete viewport.
Tests cover ordinary-map filtering, all three native/Lua modes, contact nondisclosure,
own markers, unchanged state, restart and restoration of visibility.

The shared boolean terrain query still does not implement the reference's separate
terrain/elevation mask trace, directional terrain sensor ranges or full illuminated
terrain tracing. Those remain required work for complete display LOS parity.


### Live terrain illumination

`battle_hex_illuminated(world, map, coordinate)` now derives lighting from active
fire decorations, inferno-burning units and forward searchlights. Fire and inferno
sources light their own hex and six neighbors. Searchlight terrain beams use the
current heading/torso arc, strict sixty-hex spatial reach, and the shared terrain
obstruction query. Destroyed or unavailable lamps do not contribute. Smoke,
blocked terrain and excessive woods prevent beam illumination. The terrain beam
range is separate from the existing thirty-hex unit-illumination query.

Hex visibility now passes this live lighting into optical eligibility, so visual
night range and light-amplification glare follow the same rules as illuminated
unit detection. This applies to visibility-filtered long-range maps and other
callers of the shared hex query. Queries are read-only, do not identify lighting
sources, and never acquire contacts or consume dice. Tests cover fire neighborhoods,
source removal, inferno expiry, forward/backward beams, distant lit terrain,
obstructions, unavailable units, display output and saved replay.

Directional terrain sensor ranges and the full terrain/elevation mask tracer
remain unfinished. Illumination currently uses the owned shared terrain geometry
rather than the reference's separate skyline beam trace.


Tactical display coverage: native `tactical` and Lua
`btech.unit.tactical(unit, pilot, arguments?)` share the pure Rust
`battle_tactical_map` renderer. Standard, L visibility and U underlying-terrain
modes provide terrain/elevation hexes, acquired contact IDs, own marker, palette
colors, clipping and shared display-center admission. Rendering is read-only;
native/Lua, restart, contact privacy and odd-column layout checks cover this path.
Navigation overlays, player preferences and fine
terrain/elevation LOS masks remain open.

Tactical C/T cliff overlays now share the native/Lua renderer, with three/two-level
thresholds, signed water/ice depths, viewport-bounded edges, own-marker-only
output and dark-map rejection. Regression checks include restart and unchanged
simulation/outbox state. Navigation overlays remain open.


Tactical B now renders strict landing suitability in native/Lua output, with
base grass/road, six equal-height on-map neighbors and saved circular landing
exclusions. Team exemptions apply per circle; type-9 persistence preserves the
records across restart. The own marker remains, other contacts are omitted,
and dark maps reject the overlay. Tests cover terrain, radius boundaries,
exemptions, persistence and read-only native/Lua parity. Aircraft landing physics,
permissive landing configuration and native/Lua exclusion authoring remain open.


Tactical M now shares native/Lua rendering with first-field ordering, trigger
suppression and live unblocked terrain visibility. It shows only `<>`, retains
contact IDs and moves elevations above the marker space. Rendering consumes no
dice or perception checks and publishes nothing. Tests cover clipped placement,
visibility/dark maps, persistence and unchanged state. Navigation displays and
fine terrain/elevation LOS masks remain open.


`findcenter` and `btech.unit.findcenter(unit, pilot)` now share the current-hex
navigation report, using continuous position, horizontal range and clockwise
bearing. The readout needs a conscious assigned pilot and running unit but no
scanner hardware. Native/Lua parity, restart and unchanged-state checks cover it.
The combined `navigate` display is described below.


Native/Lua `navigate` now shares a radius-two local hex map, a continuous-position
compass plot and source position, terrain, speed and heading readouts. It preserves
off-map surroundings and permits an own-hex view with failed scanner hardware.
Regression coverage includes both parities, single-hex maps, visibility, remote
centers, persistence and read-only native/Lua parity. Fine terrain LOS and aircraft
navigation remain open.


Saved player dimensions now drive native/Lua tactical and LRS displays and Lua
viewport defaults. `btech.player.view_dimensions(player, dimensions?)` queries or
replaces validated sizes inside trusted callbacks. Existing configuration columns
are preserved on save; callback rollback, restart, invalid state and display parity
are tested. Navigation remains fixed-size. Contact-filter policy remains open.


Native `mapdisplay [width height lrs-height | reset]` now provides self-service
access to saved sizing, using the same setter as Lua player preferences. It only
addresses the invoking player and works outside a cockpit. Query, mutation,
validation, switch rejection, reset, Lua agreement and restart are covered.
Contact-filter preferences remain open.


Saved unit-contact categories now apply through `contacts +` and explicit Lua
`btech.unit.contacts(unit, preferences)` tables. Player preferences can be queried
or replaced through `btech.player.contact_preferences`. Selected targets may bypass
category filters, never acquisition/visibility. Shutdown and wreck filters remain
independent. Native/Lua, restart, rollback and sizing independence are tested.
Building contacts, per-call option strings and native category editing remain open.


Per-call unit contact options d/s/e/a/t and persistent ! exclusion mode now share
native parsing with Lua `btech.player.contact_options`. Parsing returns ordered
unknown-character diagnostics and never edits saved preferences. Native/Lua,
selection, visibility, restart and grammar-boundary checks cover this path.
Building option b uses the identification-lock path described below.


Building contacts now run silent identify_building locks through native `contacts b`
and Lua `unit.building_contacts`. Hidden denied structures and all invisible
structures are omitted; ordinary denied structures retain restricted status.
State and notices roll back if a lock fails. Tests cover callback identities,
concealment, visibility, native/Lua parity and restart. Brief formatting/sorting
and finer height-aware sensor traces remain open.


Saved building inclusion is now typed and persisted with contact categories.
Lua player.contact_preferences accepts buildings="include"/"exclude"/"follow_brief";
native contacts + resolves it, while explicit b remains independent. All modes,
restart, invalid input/database state and callback rollback are tested. Follow-brief
resolves the saved unit contact mode described below.


Unit-owned brief C0..3/A0..6 settings now have native and Lua query/edit paths,
validated persistence and atomic edit notices. C1 (default) controls implicit
building inclusion; A6 disables routine notices and A2/A3/A5 restrict them to
enemies, preserving weapon-lock warnings. Acquisition is unchanged. Tests cover
modes, option composition, Lua parity, rollback, invalid state and restart.
Exact contact layouts/sorting and automatic-notice text/color variants remain open.


Native contact ordering now follows the unit brief mode. C1/C2/C3 retain up to
250 mixed rows and sort descending by range plus 10,000 for destroyed units or
20,000 for structures. Candidates enter the bounded list in saved map membership
order, then entrance order. Equal keys retain that encounter order. C0 keeps map
membership order followed by entrances. Detached Lua unit queries remain ascending
by range; this presentation policy never changes acquisition or consumes dice.
Tests cover far/near units, a nearby wreck ahead of a distant unit, structure
priority, verbose ordering, Lua-query independence and restart. Exact row layouts,
headers/footers, status columns and automatic-notice formatting remain open.


Acquired contact reports now include a five-character `status`, shared by native
S: output and Lua unit.contacts. Columns cover carried club; destruction/lamp/light;
jump/prone/standing transition; shutdown/startup/heat/inferno; and homing beacon or
electronic warfare. Each column uses priority rather than concatenating conditions.
Narc/homing uses n for friendly and N for hostile contacts; electronic indicators
prefer ECCM P, ECM E, protection p, then disturbance e. Visibility is checked before
building the report, and blocking terrain blanks all condition columns. Queries
never mutate unit state or consume dice. Tests cover native/Lua agreement,
power/posture/heat/inferno priority, beacon team case, protection versus disturbance,
visibility and restart. Towing, swarming, hull-down, aircraft control and vehicle
fire indicators await those domain systems; exact row layout is still open.


Contact views now expose `sensors.primary` and `sensors.secondary`, and native rows
prefix P/S for the eligible roles. These are live eligibility results for an
already acquired target, not the stored observation snapshot or a new acquisition
roll. Identical modes share one evaluation and report both roles consistently.
The target is omitted when neither active mode is eligible. Tests cover primary
only, secondary only, both, neither, unacquired exclusion, unchanged saved state,
native/Lua parity and restart. Weapon-arc prefixes and exact row layout remain open.


Unit and building contact reports now include a typed `weapon_arc` (front/right/
rear/left), rendered as */r/v/l. The shared biped classifier uses whole heading
degrees, rounded compass bearing and the torso's 59-degree offset, favoring right
for a merged torso pose. Coincident points use the established 180-degree display
bearing. Arm flips do not change this general indicator, and individual mounts
retain independent firing checks. Contacts compute the arc only after acquired
visibility; building lock revalidation recomputes entrance geometry. Tests cover
boundary rounding, wrapped bearings, both arm poses, torso variants, native/Lua
agreement, query purity and restart. Vehicle turret arcs await vehicle support;
exact row layout and automatic-notice formatting remain open.


Automatic contact notices now honor long A0/A2 and short A1/A3/A4/A5 formats.
They include the battlefield label and observer-relative Forward/Right Arm/Left
Arm/Rear arc. Hostile acquisition uses red and loss uses yellow in A0..A3;
A4/A5 omit styling. A6 and observer units suppress routine notices while retaining
weapon-lock loss warnings. Friendly/enemy filtering remains independent of message
length. Names are made plain and escaped before styled output. Blocking terrain
uses "something" with an uppercase label; unavailable geometry suppresses only
the routine notice. Tests cover all modes, affiliation, acquisition/loss wording,
color, escaping and preserved lock warnings. Last-identified-name snapshots across
LOS loss and additional autocontact admission policies remain open.


Each saved contact now records whether its latest observation identified the unit
through clear terrain. Scanner transitions capture that flag before removing a
lost contact. Acquisition/loss notices use the captured flag for the display name,
label case and affiliation, so a previously unidentified signal remains
"something" rather than acquiring a name during notification. This stores the
identification fact, not a historical copy of a mutable name. Live sensor roles
use a separate BattleContactSensors report type and never rewrite stored history.
Tests cover persistence, identified and unidentified loss, removal, one-time loss
events and the existing native notice modes. Other autocontact admission policies
and exact contact-list layouts remain open.


Live contact views now expose current `identified` separately from acquisition.
A seismic contact retained through blocking terrain shows "something", blank
status columns, and no friendly classification. Category filters and tactical
markers use that same disclosed affiliation. The raw target handle and kinematic
sensor report remain available. An end-to-end ridge test covers native output,
Lua, category filtering, query purity and configured restart. It also verifies
that the runtime-only seismic stopped-target setting does not block database
saves; like skill policy, it is applied from configuration on startup rather than
stored as world identity data.


Routine acquisition/loss notices now suppress shutdown and starting targets by
default. `mechprefs AutoconShutdown ON|OFF` (or toggle without a value) changes the
unit-owned inclusion preference; Lua unit.autocon_shutdown(unit, pilot, enabled)
uses the same guarded setter, and unit inspection exposes autocon_shutdown.
The setting survives restart, is independent of player contact-list categories,
and participates in callback rollback. Acquisition and weapon-lock loss warnings
are unaffected. Tests cover default behavior, starting/running targets, native/Lua
edits, authority, rollback and persistence. Additional scenario/runtime notice
level overrides remain deferred with their owning systems.


C1/C2/C3 now render compact biped unit rows from a detached shared `short_text`
field. Rows contain P/S sensor flags, torso arc, battlefield label, B movement
class, a twelve-character plain name, x/y/z, one-decimal range/speed, whole-degree
bearing/heading and five status columns. Contact views also expose label,
coordinate and elevation for custom Lua displays. Friendly labels are lowercase;
unidentified contacts retain generic names and uppercase labels. Tests fix the
column spacing, bounded name behavior, mode routing, ordering, native/Lua parity
and restart. Building row formatting, list headers/footers, color, lateral heading
and the C0 verbose layout remain open.


Building contact rows now use P/S terrain roles, the torso arc, a 23-character
plain name, x/y/z, range/bearing, current/maximum CF and two status characters.
The detached short_text is finalized after identification-lock evaluation and
revalidation. Lua building_contacts and native output share that row. Rust
battle_hex_sensor_visibility exposes the same two sensor roles used by hex_visible;
identical modes share an evaluation. Tests cover exact columns, one/both/neither
terrain sensor roles, native/Lua parity, restart and existing lock/concealment
behavior. Row colors, list framing and the verbose unit layout remain open.


Compact unit rows now highlight selected targets in bold red, taking precedence
over affiliation. Other nonfriendly contacts use bold yellow; identified allies
retain default styling. Denied building-identification rows use bold yellow.
All data-derived row text is escaped before trusted styling, including battlefield
label brackets. Native contacts returns a styled document for capability-aware
rendering. Detached short_text stays plain; Rust styled_short_text(selected) and
building styled_text share the native formatting path. Tests cover selection,
affiliation, building locks, escaping, plain-text equivalence and styled dispatch.
List framing and the C0 verbose layout remain open.

Contact modes C0–C2 now frame lists with `Line of Sight Contacts:` and
`End Contact List`, including empty lists. C3 emits only rows, and emits nothing
for an empty list. Ignored-option diagnostics precede the list without a trailing
blank line when C3 is empty. Integration tests cover all modes, query purity,
empty lists, diagnostics and persisted C3 behavior. The C0 verbose row layout
and lateral heading remain open.

C0 now renders multiline biped reports with full plain names, battlefield labels,
tonnage, range/bearing, speed/heading, coordinates, excess heat, movement type and
observer-relative arc. Destruction, shutdown, fallen and jumping notices are
independent; jumping includes the committed path heading. Like the reference,
C0 uses generic names through blocking terrain while still reporting these
physical details. Native output escapes the complete plain report and Lua exposes
`verbose_text` alongside `short_text`. Tests cover exact layout, native/Lua parity,
restart, name escaping, combined conditions and jump headings. Lateral heading
and non-biped movement classes remain open.

Biped lateral movement now uses a durable active mode and six-second pending
transition. The assigned pilot needs Maneuvering_Ace. Native `lateral` and guarded
Lua `unit.lateral` accept directional aliases, replace pending requests, and
cancel them when the active mode is requested. The simulation tick advances
changes even at rest; expiration while stopped discards the request. Restart
retains the countdown. Ground projection, domino collision relative speed and
contact/report headings use the lateral travel axis; weapon arcs retain chassis
facing. Rust exports BattleLateralMode, BattleLateralState, set_battle_lateral
and battle_lateral; Lua unit state exposes lateral. Tests cover eligibility,
commands, rollback, alias parsing, replacement/cancellation, timing, restart,
actual movement, contact displays, shutdown expiry and invalid saved countdowns.
Quad/vehicle admission and their movement rules remain pending with those classes.

Maneuvering Ace turning controls now support native `turnmode` and guarded Lua
`unit.turnmode`. Tight/normal selects the durable unit preference; empty or other
arguments report it. Mutation and occupant notification roll back together.
The assigned pilot and saved Maneuvering_Ace advantage are required, while the
control does not require running engines. Under slowdown mode two an unfinished
heading change subtracts 0.4 movement points (4.3 KPH) after the angle-based speed
multiplier. Other slowdown modes and straight movement are unaffected. Tests
cover access, query purity, native/Lua edits, callback rollback, restart and
movement comparisons across slowdown modes. Charge damage retains chassis
heading, as in the reference; domino impacts use the lateral travel axis.

Cockpit status info now shows the committed lateral direction when active and
the TIGHT/NORMAL turn mode when the assigned pilot currently has Maneuvering_Ace.
Pending lateral changes remain absent until activation. The displayed chassis
heading remains independent of travel direction. Short/export and non-info
sections are unchanged. Tests cover native/Lua parity, query purity, transition
timing, restart and loss of pilot eligibility without erasing saved preferences.

Biped bootlegger maneuvers now use native `bootlegger` and guarded Lua
`unit.bootlegger`, with a read-only Rust battle_bootlegger_modifier query.
Admission checks assigned pilot, running power, placement, 43 KPH forward speed,
intact legs and leg/weapon recycling. The situational modifier combines speed,
tonnage, submerged water and leg weapon penalties, with hip precedence and a
minimum of one. A shared piloting roll succeeds with a 90-degree heading change,
half current speed, retained desired speed and 30-second leg recycling. Failure
uses shared tactical or character falls at the modifier severity. Notifications,
XP, injuries and casualty transfers use one rollback checkpoint. Tests exercise
success and failure with seeded dice, replay, persistence, native/Lua controls,
character XP, readiness errors and complete callback rollback. Other unit classes
and detailed diagnostic-channel wording remain outside this biped slice.

Movement ETA now supports explicit integer x/y coordinates or the selected
ordinary hex, including a settling hex lock. Building, ignition, clearing and
unit targets are not defaults. Rust battle_eta is read-only; native `eta` and
Lua unit.eta publish the detached report to cockpit occupants atomically.
Horizontal range and absolute current speed determine whole minutes; speeds
below 0.1 KPH report Never. Very large estimates saturate at the reference signed
integer limit. Explicit coordinates can lie outside map bounds, and sensor
hardware is not required. Tests cover forward/reverse/near-zero speeds, hour
formatting, defaults, passengers, native/Lua output, rollback and persistence.

Compass bearing now supports native `bearing` and read-only Lua unit.bearing,
backed by Rust battle_bearing and BattleBearingReport. No arguments measures to
the selected live visible unit or selected hex of any purpose. Two integers use
the current continuous position; four supply origin and destination hex centers.
Coordinates are bounded to the current map, with the reference bearing-only
exception permitting an explicit origin on the east border. Coincident points
report 180 degrees. Unit defaults require retained acquisition and current sensor
eligibility; explicit coordinates and hex defaults do not require sensors.
Tests cover all forms, exact native/Lua text, passengers, bounds, query purity,
restart, loss of unit visibility and continuous sub-hex motion. Range/vector
commands remain open.

Native `range` and Lua unit.range_report now share validated navigation
endpoints with bearing through navigation_measurement. They accept default
targets, destination coordinates, or explicit origin/destination coordinates.
Spatial range uses continuous live unit height and signed water/ice bed heights.
Dark maps hide coordinate height differences while preserving unit-target
height. Ground range is shown only when it differs after one-decimal rounding.
The existing unit.range(first, second) geometry query is unchanged. Unlike
bearing, terrain-backed origins must name a valid tile when heights are read;
dark-map horizontal queries do not read that tile. Tests cover terrain signs,
dark masking, target visibility, native/Lua parity, query purity and restart.
Vector remains open.

Native `vector` and Lua unit.vector now share endpoint admission and range
formatting with bearing/range. Zero, two and four coordinates use default or
terrain-backed endpoints; three or six integers provide explicit x/y/z heights.
Reports include horizontal/spatial distances, compass bearing and a signed
vertical angle rounded away from zero. Text includes the vertical mark only for
default targets and explicit elevations. Vector keeps terrain heights on dark
maps, matching the reference. Tests cover every form, sign and vertical-only
geometry, bounds, explicit east-border origins, native/Lua text, query purity,
restart and target visibility.

Heading and speed now support private no-argument current-value queries.
Lua unit.heading/unit.speed likewise return the actual numeric value when the
third argument is omitted, while edits retain boolean success and cockpit
notification. Native and Lua speed requests share walk/cruise, run/flank, stop
and back names and finite numeric clamping to the current forward/reverse
envelope. Lower-level set_battle_speed remains a strict validated setter.
Queries allow cockpit passengers; changing controls still requires the assigned
pilot and existing movement checks. Tests distinguish actual from desired motion,
exercise alias/limit behavior, reject nonfinite/type errors, and cover rollback
and restart. The reference command catalog reverses the cruise/flank description;
implementation follows the actual speed table (cruise=walk, flank=run).

Cockpit-stun speed restrictions now cover native/Lua running aliases, clamped
numeric requests, direct control boundaries, permitted walking/reverse/stop and
saved recovery expiry. Rejected requests leave battle state and Lua output
unchanged. Ammunition dumping and its movement restrictions remain open.

Ammunition dumping now has owned Rust selection/cadence, native and Lua controls,
committed server ticking, persistence, live-bin ejection, existing warning
preferences, observer feedback and running/jumping restrictions. Tests exercise
invalid selectors, cancellation, replacement by all, callback rollback, saved
replay, low-capacity cadence and shutdown cancellation. The saved attempt owns
its rate phase; the C process-global phase is not reproduced. Rear-hit ignition
and remaining output fidelity still require combat integration and characterization.

Rear weapon-hit dump ignition is integrated into the shared material/tactical/character
salvo path, with rear armor transfer, conventional critical/crew effects, stagger,
one-round consumption, cancellation and report/notices. Front and nonweapon
hits do not trigger it; empty or unavailable selected bins consume no selection
dice. Tests cover typed selection, saved replay, a known rear-center impact and
nonweapon/front exclusions. Native/Lua firing uses this common salvo resolver;
wider output fidelity and additional character/transfer scenarios remain to characterize.

MASC construction now supports typed critical slots, one-ton slot mass, IS/Clan
integer installation thresholds and live critical availability. Tests cover
undersized/exact/surplus installations, repeated critical hits, mass retention,
threshold boundaries, persistence and detached Lua inspection. Runtime MASC
controls, boosted speed and failure/recovery consequences remain open, as does
supercharger integration.

MASC now has native/Lua activation, signed throttle scaling, derived movement and
XP ceilings, saved overload/recovery state, first/periodic checks, wizard-pilot
protection, permanent failure, bilateral hip loss and speed-threshold falls.
Shutdown clears activation/history while preserving failed hardware. Tests cover
native/Lua rollback, actual boosted movement, recovery restart replay, both signed
fall boundaries, cockpit inspection and failed server-save retry. Character fall
publication shares the existing resolver; further character-specific scenarios,
supercharger interaction and counter color fidelity remain open.

Supercharger controls and failure are integrated through the shared Rust booster
state and host update. Coverage includes the explicit technology flag, critical
part/mass behavior, native/Lua controls, combined-speed envelope, saved simultaneous
check order, cross-device penalty, and one-to-four ordered engine losses with
replay. The MASC tests continue to cover shared recovery and failed-save rollback.
Counter colors, further character-specific failure/casualty cases and other unit
classes remain to characterize.

Combined hot-TSM/booster checks now distinguish throttle, movement acceleration
and turning ceilings. The update conversion replaces the additional booster
adjustment when TSM is active. Persistence bounds include intermediate rounding
between toggles. Tests cover neither/either/both devices, acceleration and turning,
and hot forward/reverse throttle save/restart/cooling transitions.

- C3 hardware foundation: typed master/slave/C3i slots, construction mass, derived
  installed/live capabilities, independent five-slot master groups per section,
  slave and C3i live-slot thresholds, durable critical damage, and Lua inspection.
  Master
  groups are evaluated independently, avoiding the reference counter's carryover
  between groups when multiple computers occupy one section.

- C3i membership: six-unit networks, shared saved identity, native `c3i` and Lua
  join/leave, friendly visible target admission, live hardware/pilot/ECM checks,
  transactional notices, shutdown retention, damage/team/map disconnection,
  database replay, and malformed membership rejection.

- C3i targeting range: nearest usable peer for acquired unit contacts and clear
  terrain sightings, physical minimum/maximum checks, no network extended range,
  ordinary stealth bracket adjustment, separate source/distance diagnostics,
  shutdown/ECM exclusions without membership loss, and shared preview/shot paths.
  Verified physical minimum including hotload, fractional bracket boundaries,
  strict/extended reach, own-visibility requirements, terrain aim, and saved shot
  replay.

- C3i messaging: native `c3imessage`, Lua `unit.c3i_message`, pure recipient
  preparation, bold sender identity and cockpit echo, literal text, ASCII leading
  whitespace handling, and shutdown/ECM/unconscious recipient exclusions.
  Delivery ignores radio tuning and acquired contacts and does not alter network
  membership or dice. Verified recipient selection, all-unavailable echo,
  persistence replay, native/Lua delivery, rejected admission, and rollback.

- C3i network status: private native `c3inetwork` and pure Lua
  `unit.c3i_network`, typed peer rows, live motion/elevation/range, original-total
  armor (including rear) and structure percentages, and framed empty reports.
  Shutdown/ECM peers are omitted; unconscious peers remain visible. Verified
  no-contact inspection, damage, heading/speed, recipient privacy, purity, and
  restart parity.

- C3i target display: private `c3itargets`, detached Lua rows/text, direct/network
  visibility union, own-sensor P/S markers, clear-peer identification and condition
  disclosure, shared aim range selection, destroyed-first descending range order,
  selected/opponent colors, and unknown-name masking. Verified network-only
  sightings without acquisition, per-row visibility isolation, source/range
  agreement, native/Lua parity, purity, and restart.

- Classic C3 membership: native `c3`, Lua `unit.c3`, independent saved group
  identity, shared C3/C3i admission/transaction engine, working-master capacity
  (4/7/10/12 units), multiple masters per chassis, and master-priority capacity
  reduction. Verified slave-only rejection, expansion, hard limit, multi-master
  damage, persistence replay, family independence, native/Lua rollback, and
  shutdown/team lifecycle.

- Classic C3 range assistance: shared unit/hex aim path, explicit network-family
  diagnostics, priority over simultaneous C3i, fallback after disconnection or
  hardware loss, and temporary capacity reduction for unavailable masters.
  Temporary views retain the requester and masters without deleting membership.
  Verified priority, shutdown capacity changes, physical minimum/maximum limits,
  terrain aim, hardware fallback, and actual-shot restart replay.

- Classic C3 messaging: native `c3message`, Lua `unit.c3_message`, pure recipient
  preparation, shared C3/C3i delivery engine, independent audiences, and temporary
  master-capacity checks after shutdown/ECM/consciousness filtering. Verified
  unconscious-master capacity loss, family separation, retained membership/dice,
  save/load replay, native/Lua delivery, and transactional rollback.

- Classic C3 displays: private `c3network` / `c3targets`, detached Lua
  `unit.c3_network` / `unit.c3_targets`, and shared typed reports/rendering with
  C3i. Status and target peers use active master capacity after shutdown/ECM
  filtering, retaining unconscious masters. Verified capacity shrinkage without
  membership loss, independent C3i sightings, no contact acquisition, native/Lua
  privacy, pure state/RNG, and restart parity.

- Laser heat-sink designation: `LaserHS_Tech` is preserved as template metadata,
  matching the reference's recognized but behaviorally unused flag
  (`unit/template_flags.c`, `unit/mech_status_types.h`). It does not alter sink
  grouping, mass, cooling, or critical damage. Verified single/Clan sink parity,
  four unchanged Night Gyr assets, grouped losses, and database restart.
  NightGyr-B retains its incomplete sink installation rejection.

A fresh construction audit on 2026-09-10 accepts 1,174 of 1,745 unchanged assets,
up from 1,170 before accepting the laser-sink designation. The four additions are
NightGyr-Prime, NightGyr-A, NightGyr-C, and NightGyr-D; no previously accepted
asset regressed. This is construction coverage, not complete gameplay parity.

- Template range overrides: independent tactical, LRS, detailed-scan, and radio
  ranges, zero/missing default selection, and sensor-critical halving/disabling.
  Existing live consumers and Lua inspection share the derived values. Verified
  boundary admission, digital radio reach, damage, invalid values, defaults,
  query purity, and database replay. RadioType capabilities are described below.

The range-override audit accepts 1,180/1,745 unchanged assets, adding ASN-30,
CLNT-5U, FLE-15, HA1-OC, PNT-12A, and WTH-2A with no construction regressions.

- Explicit RadioType: channel count, relay/info/scan/no-digital capability bits,
  quality defaults at zero, native/Lua mode admission and list formatting, digital
  relay-path info, and analog-only receiver/relay exclusion. Analog scanning uses
  persisted receiver dice, converges channel frequencies, and publishes ordered
  feedback in the transmission transaction. Verified defaults, malformed settings,
  high channels, digital/zero/matched exclusions, muted shutdown scanning,
  saved replay, and Lua callback rollback. Manual-frequency audits exclude scan
  channels. Exact-frequency search avoids an undefined division in the reference.

Construction now accepts 1,189/1,745 unchanged assets, adding ARC-5S, six CP
variants, HKO-1C, and JN-G8A; no previously accepted asset regressed.

- Chassis anatomy foundation: biped/quad classification, chassis-specific section
  names and leg roles, critical capacities, quad asset decoding independent of
  field order, and typed chassis diagnostics. Verified an unchanged Scorpion
  asset, four-leg equipment inspection, mixed/duplicate-heading rejection,
  template serialization, and continued rejection of unsupported quad simulation.
  Quad mobility, damage, hit tables, attacks, controls and lifecycle remain open;
  the decoder does not make these units playable.

The anatomy audit decodes 28 quad assets for diagnostics. Live construction
remains 1,189/1,745, with no previously accepted assets regressing.

- Quad mobility components: four-leg damage/flooding counts, distinct 0/1/2/3+
  leg-loss speed and difficulty rules, repeated hip halving, same-leg actuator
  suppression, and shared gyro disablement. Chassis skill identity now serves
  checks and XP, with the quad bonus separate from damage. Shallow immersion
  includes functioning front-leg sinks. Component tests cover all sixteen leg
  loss patterns, flooded support, critical combinations, and serialized replay.
  Live quad construction and movement/combat dispatch remain gated/incomplete.

- Quad movement components: doubled ground acceleration, proportional lateral
  speed loss, intact-quad lateral admission, and active-offset clearing on leg
  destruction. Shared movement now honors Speed Demon for acceleration/braking.
  Verified live biped acceleration and restart, quad signed-speed calculations,
  support/advantage admission, and saved lateral cancellation state. Quad live
  construction remains gated while the remaining runtime rules are completed.

- Quad support components: shared unavailable-leg counts for mobility, standing
  and landing, automatic intact-quad standing, three-leg support failure, and
  chassis-aware flooding/balance classification. Tests cover all sixteen loss
  patterns with destruction and flooding, hip/gyro combinations, biped support
  regression, and automatic-check dice/XP and power behavior. Live quad
  construction remains gated while combat and remaining runtime rules are open.

- Quad hit-table components: explicit chassis selection at every hit-table
  caller, directional punch/kick distributions, shared weapon distribution,
  and chassis-aware head-graze rerolls. Exhaustive physical rolls and weapon
  distribution comparisons cover all arcs; seeded head-hit tests verify stun
  modes, exact dice consumption and serialized replay. Live quads remain gated.

- Quad firing geometry: mandatory chassis input for mount queries, four-leg
  independence from torso offsets, retained front-leg side arcs and rear-mount
  precedence, and torso-rotation rejection. Exhaustive compass tests cover
  every section, torso pose, arm-flip state and mount direction against the
  shared biped geometry. Live quad construction remains gated.

- Quad physical components: front-leg kick/trip selection throughout targeting
  and recovery, three-leg minimum support, surviving-hip checks, and rejection
  of punch/club/hand-weapon attacks. Component tests cover both sides and all
  sixteen flooded support patterns plus every hip and forbidden arm attack.
  Full live quad combat remains gated until remaining runtime integration.

- Quad charge components: four-leg selection support with at most one unavailable
  leg, flooded-support rejection for bipeds, and one-way defender control bonus
  distinct from mutual raw-skill comparison. All loss patterns and chassis/role
  combinations are covered, including serialized skill calculations. Live quad
  construction remains gated.

- Quad weapon components: four-leg actuator accuracy, submerged front-mount
  restrictions in unit/hex firing, and intact/damaged prone support rules.
  Tests exhaust all sixteen destroyed-leg patterns across mounted sections
  plus foot/upper/lower/hip accuracy combinations. Live quad construction
  remains gated while runtime integration continues.

- Quad MASC failure: all four hips are damaged; permanent failure immobilizes
  and prevents upright landing. Component tests distinguish ordinary quad hip
  damage from MASC seizure and verify shutdown/serialization persistence.

- Quad display integration: chassis labels in armor/weapon status, scans,
  compact weapon exports, limb recovery and critical/explosion messages; front
  actuator observer feedback uses leg roles. Component armor/export tests
  verify front/rear labels and unchanged snapshots. Side-torso destruction
  was inspected and already correctly removes the attached front leg.

- Quad pivot/landing integration: bootleggers check and recycle all four legs,
  reuse derived actuator accuracy, and reject front support damage/recovery.
  Jump landing warnings and cockpit critical notices use chassis leg roles.
  Component tests cover each leg's actuator, destroyed, flooded and busy state.

- Live quad construction enabled: nineteen additional unchanged assets accepted,
  1,208/1,745 total with no prior acceptance regressions. Live Scorpion scenarios
  cover startup, placement, acceleration, automatic standing after a fall, weapon
  resolution, front-leg kick recovery and database replay with world validation.
  This supersedes earlier quad-construction gate notes in this chronological log.
  Nine quad assets still fail supported-equipment validation; broader parity and
  other vehicle classes remain incomplete.

- Live quad club admission: acquisition rejects quads before arm/terrain checks;
  direct, native and Lua paths preserve state and share the reference message.

- Chassis location selectors: shared command parsing for dump and native/Lua
  pod removal, quad front/rear compact/full names, strict anatomy separation,
  and chassis-capacity dump slots. Live quad bin selection tests cover aliases,
  out-of-capacity and wrong-section rejection without mutation.

- Live quad pod rules: inspection uses chassis labels; direct/native/Lua swatting
  is rejected consistently without changing state, matching the reference rule.

- Rejected quad engine audit: BGS-1T 6/5/1 and BGS-2T 6/4/2 produce
  mixed engine-family evidence in the reference; GOL-3S includes a rear-leg
  engine slot. Explicit count/location diagnostics and asset tests document
  why these remain rejected. Mixed-family engine behavior remains open.

- Asymmetric engine support: 6/5/1 and 6/1/5 resolve to effective XL;
  6/4/2 and 6/2/4 resolve to effective XXL for current mass/BV/combat behavior.
  BGS-1T and BGS-2T now construct unchanged. Numeric mass, critical damage and
  serialization tests cover these layouts. This supersedes their rejection
  in the preceding audit; out-of-torso engines and economy/repair remain open.

The asset audit now accepts 1,211/1,745 templates, adding AXM-1N, BGS-1T
and BGS-2T with no previously accepted templates regressing.

- Low declared double-sink cooling: removed the extra twenty-point admission
  minimum while retaining even capacities, ordinary bounds and group validation.
  Verified exact cooling and serialized mass/rates for capacities 10..20.
  Five additional unchanged assets construct; 1,216/1,745 with no regressions.

### A-Pod equipment and combat

Added both A-Pod identities to the Rust catalogue, Lua annotations and catalogue
snapshot. Construction tests cover FireScorpion-1/2 and SRC-3C/5C; accounting tests
cover half-ton mass and both defensive and offensive Battle Value. Live combat
checks a successful zero-damage hit, thirty-second recovery, atomic rejection of
premature refiring, world validation and database round-trip for both variants.

The game-directory audit now constructs **1,235 of 1,745 assets**, up nineteen with
no regressions from the low-cooling audit. The additional assets are Daishi-S,
Das-B, Dasher-B, FireScorpion-1/2, Kos-A, Koshi-A, Lok-P, Loki-H, Loki-Prime, Man-C,
ManOWar-C, Nob-P, NoboriNin-Prime, OTT-9S, SRC-3C/5C, Ull-C and Uller-C. Broader
vehicle, artillery, autopilot, repair and economy integration remains unfinished.

### Artillery flight and impact patterns

Implemented validated, serializable committed-second flight cursors and standard,
cluster, smoke and mine impact patterns. Tests cover flight-time boundaries,
no random draws before arrival, restart replay, map-edge bomblet conservation,
weighted placement, extreme wind, local mine effects, smoke duration draws,
invalid-input rollback and prevention of repeated arrivals. The implementation
uses native Rust geometry and explicit random streams and introduces no C bridge.

The following stages connect these domain rules to live firing and persistence.

### Artillery arrivals in world transactions

Connected flight arrival patterns to tactical damage, smoke overlays and minefield
creation. Added a host action for character injury, notices and evacuation, with
atomic rollback of the cursor, world and staged effects. Tests cover direct and
neighbor damage packet counts, cluster packets across an occupied field, exclusive
height limits, smoke replacement, preservation of existing mines, database replay,
character publication and late rejection after earlier damage/random draws.

World-owned queues and live firing are described in the following stages.

### Persistent artillery queues and server service

Added bounded map-owned launch queues, sparse database storage, load validation,
map-purge cleanup, reload retention and automatic committed-second ticking.
Character arrival publication and every queue update share the existing world
transaction. Tests verify ordered simultaneous arrivals across database restart,
rounds surviving a pending-removal shooter, rollback after a later arrival fails,
rejection of corrupt saved cursors, and idle-server retry after an injected database
delete failure. Live launch admission is described below.

### Artillery catalogue and dedicated hit calculation

Added eight typed artillery catalogue entries and their Lua identity annotations,
plus a dedicated range/visibility/spotting/correction calculation and artillery
pilot-skill lookup. Tests cover raw range boundaries, cannon map-sheet ranges,
submerged/out-of-range sentinel targets, signed spotting truncation, correction
values, independent skill selection and protection against conventional instant
fire. The configured launch action below uses the dedicated calculation.

### Typed artillery ammunition

Added Smoke/Mine ammunition identities and artillery-specific Cluster decoding,
with weapon-aware template flag resolution, mounted selection, payload conversion,
serialization and compact display letters. Tests cover all eight artillery
families, every payload, conflicting flags and preservation of conventional LB-X,
Artemis and Narc meanings. LB-X controls retain their equipment restriction.
The live launch action below supports all four payloads.


### Live artillery launch integration

Enabled structurally valid artillery construction and connected native/Lua firing
to dedicated artillery aim and persistent flights through shared weapon
expenditure. Added saved miss correction, friendly observation and correction
reset on targeting/observer changes. Integration tests cover native/Lua state
parity, permission/target/recycle rollback, ammunition/heat/recovery, all four
payloads, delayed arrival, correction/reset and restart during and after flight.
Cluster switching and detailed artillery broadcasts are described below. Broader
artillery and unit-class parity remains unfinished.

The live-launch audit constructs **1,242 of 1,745 assets**, up seven with no
regressions from the A-Pod audit. Newly admitted assets: Nag-A, Nag-B, Nag-C, Nag-D, Nag-P, Naga-D, TDK-7KMA.


### Artillery cluster selection

Added the native `cluster` command, Lua `unit.cluster`, public Rust control,
annotations, access policy and cockpit help. Shared guards preserve authorization,
readiness and selection parsing. Tests verify native/Lua parity, callback rollback,
no expenditure on selection or shortage, persistence, live cluster arrival,
recycle rejection, incompatible weapons and smoke/mine preservation.


### Artillery observer broadcasts

Flights now own validated weapon identity and derive damage from the catalogue.
Added projectile/direction launch messages and visible-hex arrival messages for
standard, cluster, smoke and mine payloads. Audience and location rendering is
shared with mine explosions. Tests verify native launch text, arrival payload
text, occupied/empty hex labels, recipient filtering, weapon validation and
identical reports/outboxes after restart without a live shooter.


### Artillery observer admission

Connected artillery to the common live observer-link validator and required
observer hex visibility. Observed launches now use indirect aim, including for
hexes visible to the shooter, and retain their separate admission behavior.
Tests cover native/Lua equality, saved launch state, invalid links and targets,
blocked sight, cross-map observers, no expenditure/notices on failure and
correction reset for observer followers.


### Artillery hotloading

Artillery supports the shared native/Lua hotload control and template flag while
retaining its separate aim rules and payload selection. Rolls of two or three jam
the launcher without ammunition, heat or a queued round; other rolls use ordinary
artillery launch expenditure. A jam or recycle timer prevents mode changes.

The first hotloaded artillery weapon critical uses catalogue damage when a live
ordinary ammunition bin exists. Special-only or empty supply does not explode,
and this critical check does not consume ammunition. Packet multiplication uses
a minimum count of one for weapons without missile racks.

Tests cover all eight families' hotload eligibility, template selection and saved
state, native/Lua launch and jam parity, expenditure, queued-round admission,
blocked mode changes, database replay and critical explosion eligibility.


### Ground-vehicle asset foundation

Extracted the shared bounded template syntax from BattleMech validation and added
separate ground-vehicle movement and section identities. `BattleVehicleTemplate`
preserves hull/turret layouts, equipment slots, mode flags and unit metadata.
Tracked, wheeled, hover and stationary definitions do not reuse Mech anatomy.
The configured-directory reader retains size and path-confinement checks.

`btech_vehicle_audit` inspects explicitly named assets and always reports vehicle
simulation support as false. The initial audit decoded **222 of 235** Vehicle
assets. Twelve assets have equipment fields outside the current syntax contract;
J-27_Transport has an unexpected Rotor section. These are diagnostics, not silently
normalized definitions. Equipment interpretation, vehicle construction, movement,
combat, persistence and native/Lua gameplay integration remain to be implemented.

Tests cover four movement classes, optional turrets, repeated weapon slots,
metadata/mode preservation, saved definition round-trip, malformed input,
size/path bounds, class separation and existing Mech parser behavior.

The shared-parser extraction preserves **1,242 constructible Mech assets**, with
no regressions in the complete game-directory audit.


### Ground-vehicle equipment resolution

`BattleVehicleLoadout::resolve` interprets each vehicle weapon slot as a complete
weapon. Hull-face and slot order preserve weapon numbering. A Demolisher's two
adjacent AC/20 slots resolve to two weapons, without imposing Mech critical-slot
counts. Bins remain independent, with shared capacity, quantity and mode checks.

Weapon, ammunition and system records carry a typed location parameter so vehicle
faces cannot be mistaken for Mech limbs. Shared mount/bin constructors own mode,
one-shot, brand and supply validation; Mech allocation and split-critical rules
remain in the Mech resolver. Vehicle equipment resolution does not validate mass,
engine/system installation or admit a live vehicle to combat.

The audit resolves equipment for **220 of 235 vehicle assets** (222 parse).
RadioTower's Stinger ammunition is unsupported, and Svantovit-Streak has an
oversized ammunition bin. The other thirteen retain their parsing diagnostics.
All audit entries still report simulation support as false.

Tests cover the full existing weapon catalogue in single vehicle slots, artillery
payload/hotload selection, brands, systems, optional turrets, independent bins,
capacity rejection, unknown equipment, invalid modes and definition round-trip.


### Ground-vehicle engine diagnostics

`BattleVehicleTemplate::engine` reports nominal output, suspension-adjusted mass
rating, catalogue availability, powerplant family, engine mass and the hovercraft
minimum. Mechs and vehicles share speed-to-rating arithmetic, catalogue lookup
and fixed-point rounding. Vehicle rules apply fusion shielding before XL, XXL,
Light or Compact modifiers; combustion engines double base mass without shielding.
Conflicting technology flags retain the reference's precedence.

Tracked/stationary suspension is zero, wheels subtract twenty, and hovercraft use
the tonnage brackets 40/85/130/175/235. Hover engine mass is at least one fifth of
nominal tonnage. Missing catalogue ratings remain explicit in the report even
when the hover minimum supplies a mass; they do not establish valid construction.
Nonfinite, negative and overflowing input is rejected.

The vehicle audit includes these engine diagnostics. Tests cover real tracked,
wheeled and hover assets, technology precedence, shielding/rounding order,
suspension boundaries, missing ratings, hover minimums and saved definition replay.
Complete vehicle mass, structural validation and live simulation remain unfinished.

### Ground-vehicle intact mass accounting

- Added Rust-only `BattleVehicleMass` diagnostics for hull structure/materials,
  controls, turret weapons, hover components, armor, whole vehicle equipment,
  cooling, cargo, installed bins, and listed ammunition.
- Reuses the existing engine catalogue and half-ton rounding, while retaining
  quarter-ton vehicle controls/turrets and vehicle-specific system mass.
- Reports design and listed-load totals separately, rejects missing engine
  catalogue entries and invalid cargo, and exposes results in `btech_vehicle_audit`.
- Corrected vehicle XXL engine selection to the reference asset flag `XXL_Tech`.
- This milestone does not admit vehicles to live play or certify construction.
- Vehicle asset audit: 219 of 235 selected assets produce complete mass reports.
  Thirteen fail template decoding, RadioTower and Shamash lack engine catalogue
  ratings (0 and 58), and Svantovit-Streak has an overfilled ammunition bin.

### Ground-vehicle material state and snapshot replay

- Added owned `BattleVehicle` protection and ammunition state with validated JSON
  replay and derived equipment, separate from Mech-specific runtime fields.
- Shared damage phase/results now support vehicle section identities. Hull-face
  loss destroys a vehicle; turret loss clears turret armor and bins but leaves the
  hull alive. Repeated hits on lost sections return their unabsorbed damage.
- Tests cover phased armor/internal damage, all four hull destruction faces,
  turret loss, ammunition expenditure, invalid snapshots, and deterministic replay.
- World/SQLite integration and full vehicle combat remain unfinished.

### Ground-vehicle world ownership and SQLite storage

- Added world-owned vehicle records, common unit identities and checked creation
  on unused live things. Mech-only runtime iteration remains separate.
- Added bounded versioned SQLite snapshots, transactional registration writes,
  load validation, and explicit cleanup during owning-object purge.
- Storage tests cover damaged turret/bin replay, copy-on-write isolation,
  registration-failure rollback, unsupported creation targets, corrupt/oversized
  records, and purge with foreign keys disabled.
- Vehicle battlefield placement, commands, movement and full combat remain pending.

### Ground-vehicle administrative placement

- Shared placement/removal entry points now handle owned vehicles, preserving
  material state and coordinating containment with saved positions and identities.
- Battlefield slots are allocated across both Mechs and vehicles, with same-map
  stability and first-vacancy reuse. World validation checks mixed-class uniqueness.
- Tests cover mixed placement, SQLite replay, invalid-coordinate atomicity,
  inconsistent containment rejection, occupied-map reload rejection and map purge.
- Vehicle locomotion, operator controls and combat admission remain pending.

### Ground-vehicle native and Lua administration

- Native and Lua unit creation now dispatch by declared Mech/Vehicle asset type
  through one shared checked constructor, with confined reads and no parse fallback.
- Lua state inspection returns detached class-specific projections, distinguished
  by `kind`; vehicle snapshots explicitly report that simulation is not active.
- Tests cover native/Lua creation parity, placement, callback rollback, detached
  tables, removal, replay after asset deletion, class rejection and confined reads.
- Vehicle operator controls, movement and combat remain unfinished.

### Ground-vehicle cockpit and power lifecycle

- Shared cockpit assignment/release supports vehicles, checks cross-class pilot
  uniqueness, and reconciles departure, relocation, and object purge.
- Vehicle startup/shutdown uses the existing native/Lua entry points, with saved
  thirty-second startup or five-second override and movement-specific messages.
- Server heartbeat detects pending vehicle startup. Tests exercise real-server
  failed-save retry, countdown replay, callback/output rollback, override authority,
  invalid snapshot rejection, native pilot/leave and shutdown after hull loss.
- Motion, contacts, startup observer broadcasts and combat remain pending.

### Ground-vehicle motion calculation

- Extracted shared ground turning and acceleration from the Mech update loop.
- Added intact vehicle motion proposals with terrain speed costs, tracked/wheeled
  paving bonuses, reverse travel, high-speed road turns, FASA turning, slowdown,
  Speed Demon acceleration and map movement scaling.
- Tests check terrain-adjusted targets, braking and reversal, road/hover differences,
  advantage/rate effects, high-speed turning and rejected invalid inputs.
- Proposals do not commit position or bypass terrain hazards. Vehicle terrain entry
  and live movement integration remain pending.

### Live ground-vehicle controls and traced movement

- Added durable continuous motion, shared native/Lua speed and heading controls,
  named throttle requests, cockpit readouts and active server movement updates.
- Shared movement transactions now advance vehicles on admitted level terrain,
  trace proposed paths, update saved hex coordinates and stop at map boundaries.
- Tests cover acceleration, native/Lua requests and rollback, deterministic SQLite
  replay, continuous/hex positions, a real server movement update, edge stops and
  unsupported-hazard stops that preserve protection and ammunition.
- Unresolved terrain/elevation, occupied hexes and maps with mines explicitly stop
  travel. Vehicle hazard effects and complete combat remain unfinished.

### Hovercraft water travel and vehicle geometry

- Added class-aware support height and admitted hover travel across normal water
  and level-zero shorelines independently of water depth.
- Shared range/elevation APIs now support vehicles and mixed Mech/vehicle queries.
- Underlying terrain drives movement costs and support height; smoke does not hide
  terrain restrictions, while fire overlays remain explicit unsupported hazards.
- Added level-mountain admission with its existing speed cost. Tests cover water
  depth changes, shoreline exit, smoke, replay and mixed-class spatial range.
- Ice, bridges, flooding, slopes, mines, collisions and combat remain pending.

### Vehicle one-level slope travel

- Ordinary one-level elevation changes now admit vehicle travel and subtract
  two movement points of speed per crossed slope, preserving throttle intent.
- Consecutive path steps compare their own support heights instead of comparing
  every crossed hex against the original starting height.
- Tracked reverse slopes are admitted without a control check. Wheeled/hover
  reverse checks share Mech control logic, use vehicle fall impacts and restore
  uphill positions after damage. Larger cliffs use avoidance checks and vehicle
  fall consequences.
- Tests cover climb/descent speed loss, reverse/configuration behavior, saved replay,
  and stopping before a two-level cliff. Full hazard and combat integration remains.

### Vehicle bridge travel

- Admitted ordinary deck travel and hovercraft passage from water beneath clear
  spans, keeping surface elevation separate from deck elevation.
- Persisted underpass state with vehicle-class and map validation.
- Tests cover deck slopes, varying span heights, water exits, saved replay and
  rejection of inconsistent underpass state.
- Low-span clearance checks now apply avoidance or fall damage and restore
  underpass position. Ground vehicles can share occupied hexes without stacking.

### Hovercraft ice surfaces

- Admitted hovercraft ice travel without movement-triggered fracture, at surface
  height independent of depth.
- Tests cover ice/water/shore and ice/bridge-underpass transitions, unchanged ice,
  speed, geometry, smoke and saved replay.
- Wheeled hazard tests also verify that smoke does not bypass ice restrictions.
  Tracked/wheeled fracture and casualty handling remain pending.

### Replayable vehicle dice

- Added private per-vehicle dice to owned state and the existing shared domain
  dice operation, using the same generator as Mechs.
- Verified independent streams, checkpoint isolation, SQLite restart and failed
  save retry, bounded request validation and rejection of unknown algorithms.
- Lua continues to expose only the public vehicle projection. Creation parity
  compares gameplay state independently of fresh random seeds.
- Vehicle hazard checks and failure consequences remain to be integrated.

### Vehicle piloting checks

- Shared skill-target and control-roll APIs now select vehicle Drive/extended
  locomotion skills and consume private vehicle dice.
- Added small-cockpit and absent-character-pilot penalties; inactive/unconscious
  vehicles fail without consuming dice and unavailable objects are rejected.
- Tests cover class skill selection, earned skill levels, default crew, extreme
  modifiers, blocked crew and saved replay.
- Vehicle mobility criticals, blindness, XP/notices and hazard consequences remain
  to be connected before the movement admission boundaries can be removed.

### Standard vehicle hit resolver

- Added a typed standard vehicle hit result and direction/2d6 resolver with turret
  presence/loss fallback and conditional through-armor critical eligibility.
- Covered hull/turret armor thresholds, full armor, unarmored faces, critical modes
  and stationary/critical-proof exemptions without consuming unnecessary dice.
- Tests enumerate all standard locations and verify boundary and RNG behavior.
- Alternate FASA/advanced tables, dug-in/combat-safe routing and applied damage
  consequences still need live combat integration.

### FASA vehicle hit resolver

- Added explicit motive-speed loss, immobilization and turret-lock outputs alongside
  location and critical candidates, using shared location and armor gating rules.
- Preserved friendly/shielding options, hovercraft side asymmetry, configured
  critical thresholds and critical-proof precedence. Existing conditions suppress
  duplicate motive/turret effects.
- Exhaustive roll/direction/configuration tests include turret loss, condition
  suppression, critical-proof equipment and conditional dice consumption.
- Persistent application of those effects, advanced tables and live vehicle combat
  are still unfinished.

### Persistent vehicle motive effects

- Added saved motive speed loss and immobilization while preserving construction
  data; exposed a checked world operation for applying motive hits.
- Live driving, acceleration and shared throttle/heading controls use damaged
  maximum speed. Loss clamps forward/reverse motion; immobility halts all motion.
- Tests cover replay, Lua projection/throttles, speed exhaustion, immobility,
  unavailable objects and invalid saved damage/motion.
- Vehicle attacks still need to apply these effects in their damage transaction.
  Turret-lock persistence, other criticals and complete combat remain pending.

### Vehicle turret facing and locks

- Added hull-relative saved turret facing, persistent lock damage and current
  hit-condition projection for suppressing repeated effects.
- Added native/Lua turret query/control with cockpit, power and lock checks.
  Immobilized hulls can still rotate undamaged turrets.
- Tests cover hull turns, input validation, native/Lua parity, rollback/restart,
  turret locks/loss and impossible saved state.
- Jamming, automatic tracking, weapon arcs and full vehicle combat remain pending.

### Vehicle firing geometry

- Added hull-face and narrow turret weapon arcs using saved hull/turret facing.
- Unit-index queries reject invalid/unplaced weapons and exclude lost sections
  or destroyed hulls; locked turrets retain their existing firing arc.
- Tests cover boundaries, rounded bearings, wraparound, rear-mount handling,
  locked-turret hull turns, replay and turret loss.
- Vehicle firing transactions and remaining targeting/ammunition/critical rules
  still need integration.

### Vehicle gunnery targets

- Shared weapon-index gunnery queries now accept vehicles, using basic conventional
  or extended weapon-family skills with saved earned levels.
- Preserved crew fallback and index validation without granting firing authority.
- Tests cover family selection, class-specific basic skills, signed targets,
  read-only behavior, restart and absent crew.
- Separate gunners and the complete vehicle firing transaction remain pending.

### Vehicle readiness and recycle persistence

- Added saved weapon countdowns and expended one-shot launchers with mechanical
  readiness and atomic normal-cycle ammunition reservations.
- Connected idle vehicle recycle timers to committed server updates; shutdown
  pauses them and section loss removes obsolete timers. Lua exposes saved state.
- Tests cover ammunition, energy/one-shot behavior, replay, invalid snapshots and
  failed-save retry on a real stationary server vehicle.
- Failed Streak lock reservations recycle without ammunition or one-shot expenditure,
  with readiness checks and saved replay. Ground vehicles do not use live heat.
- Targeting and Streak lock resolution, alternate modes, jams and hit/miss effects
  remain pending.

### Shared vehicle movement aim

- Added vehicle shooter movement with shared Mech ground arithmetic and original
  construction speed thresholds after damage.
- Shared target queries combine vehicle speed bands and immobility, and now supply
  existing Mech aim calculations too.
- Tests cover speed boundaries, reverse/turning behavior, turret rotation, saved
  replay, motive damage, shutdown, unconscious crew and stationary construction.
- Full vehicle contact acquisition, aim and attack transactions remain pending.

### Stationary zero propulsion

- Admit stationary zero-rating construction without inventing an engine catalogue
  entry; mobile and positive-rating checks remain enforced.
- Tests cover mass accounting, admission, startup, rejected driving, turret
  control, weapon recycling and persistence replay.
- RadioTower still requires Stinger ammunition support before its full asset loads.

### Stinger ammunition foundation and RadioTower

- Added typed Stinger bins/mount selections, status marker, Lua state annotation,
  seven-hex additional reach and ordinary missile damage groups.
- Mech firing rejects ground/coordinate targets atomically and admits jumping
  targets through ordinary checks; tests verify full shot replay.
- The original RadioTower asset now passes equipment/mass admission, startup,
  turret controls and persistence; this closes the earlier Stinger loading gap.
- Stinger selection commands, aerospace/drop-specific accuracy and full vehicle
  attack integration remain pending.

### Native and Lua Stinger selection

- Added transactional `stinger` and `btech.unit.stinger` controls for Mechs, sharing
  weapon readiness and one-shot restrictions.
- Updated command inventories, Lua annotations and pilot help.
- Tests cover native/Lua parity, replacement/toggling, missing matching ammunition,
  rejected controls, callback rollback and restart.
- Vehicle selection and full vehicle combat remain pending.

### Vehicle live ammunition modes

- Added independent saved ammunition selections and wired them into readiness,
  matching-bin reservations and Lua state.
- Native/Lua Stinger selection now supports vehicles through the existing command
  transaction, with vehicle-owned state and authorization.
- Tests cover replacement of initial template selections, mode-specific expenditure,
  no ammunition substitution, malformed snapshots, callback rollback and restart.
- Other vehicle mode commands and full attacks remain pending.

### Shared ammunition control dispatch

- Replaced the vehicle-specific Stinger toggle implementation with common class
  dispatch for ammunition authorization and storage.
- Enabled vehicle LB-X, semi-guided, precision, flechette, armor-piercing, caseless
  and incendiary selection through existing native/Lua operations.
- Tests cover every mode, matching supply, native/Lua equality, callback rollback,
  restart, expenditure and recycle rejection.
- Vehicle attack effects, loader failures and other equipment controls remain open.

### Vehicle equipment-loss state

- Added validated saved slot losses, independent of section damage and construction.
- Integrated losses into weapon readiness, firing arcs, bin availability and
  recycle cancellation; Lua state exposes the lost slots.
- Tests cover isolated mount/bin damage, idempotence, unaffected neighboring
  equipment, invalid snapshots, unavailable objects and restart replay.
- Critical selection, explosions and system/crew consequences remain pending.

### Vehicle weapon-critical candidates and selection

- Added section-local surviving weapon candidates and uniform selection from
  saved vehicle dice, with no roll when no candidate remains.
- Tested composition with mount loss, live recycle/empty-ammunition eligibility,
  exact stream advancement, invalid objects and persisted replay.
- Weapon-specific critical consequences and complete table dispatch remain pending.

### Ground-vehicle critical table selection

- Added typed standard/FASA/advanced reports with rule precedence, section-specific
  advanced outcomes and standard motive/turret branches.
- Preserved critproof/combat-safe/disabled suppression and stationary dice ordering.
- Tests cover every advanced roll and face, severe-table branches, existing damage,
  exact dice advancement and persisted replay.
- Consequence application, table-specific engine/fuel/containment behavior and
  other vehicle-class critical tables remain pending.

### Vehicle control damage

- Added persistent driver/sensor penalties and section-local stabilizer loss.
- Driver damage contributes to live piloting; per-weapon control arithmetic combines
  sensor penalties with doubled movement for damaged stabilizers.
- Tested repeat hits, saturation, section isolation, turning/flank movement,
  native stored crew skill separation, Lua state and SQLite replay.
- Full vehicle aim and critical dispatch remain pending.


### Vehicle crew stun and commander damage

Crew stun lasts 60 simulation seconds. Repeated hits restart that countdown,
including while powered off. Stun reduces an excessive forward throttle to cruise
minus 0.1 kph and prevents flank-speed requests and weapon reservations. Reverse,
heading and turret controls remain available. Hull destruction cancels recovery.
Commander hits add one piloting and one gunnery penalty and stun the crew. The
shared firing penalty is exposed as `gunnery_damage`; `crew_stun_remaining` is
available in Lua state and saved with the vehicle.

Tests cover restart, repeated hits, control admission, destruction and recovery
while powered off, including rollback and retry after a failed server save.
Critical-table dispatch and full vehicle targeting and firing remain unfinished.


### Vehicle temporary weapon critical failures

Section-local weapon-jam criticals select an intact weapon that has no existing
failure and disable it for 60–120 powered seconds. Selection and duration use
the saved vehicle dice stream; sections with no candidates consume no dice.
Ballistic weapons jam and other families short out. A critical replaces any
ordinary recycle countdown, without spending ammunition or destroying equipment.

Recovery uses the existing weapon timer, pauses while shut down, and reports
that the weapon is operational again. Individual mount or section loss removes
the temporary failure. Lua exposes `weapon_failures` beside `weapon_recycle`.
Tests cover selection, repeat-hit exclusion, exact dice advancement, saved replay,
shutdown pauses, recovery, invalid snapshots and destruction cleanup.
The older main-weapon-jam critical, turret jams and automatic critical dispatch
remain separate unfinished work.


### Vehicle turret jams and crew repair

Turret jams preserve facing and firing arcs while blocking rotation. A second
jam becomes a permanent lock; directly locking a jammed turret clears the jam.
Native `fixturret` and Lua `btech.unit.fixturret(unit, pilot)` start 60-second
attempts. Each request schedules an independent attempt (up to 256 pending).
Firing remains blocked until every attempt has elapsed. Timers continue while
powered off; completion requires a running vehicle and conscious current pilot,
and cannot remove a permanent lock. Turret or hull destruction cancels attempts.

Tests cover repeat attempts, restart replay, failed completion after shutdown,
lock promotion, firing admission, invalid snapshots, native/Lua rollback and
destruction cleanup. Full critical dispatch remains pending.


### Atomic vehicle critical resolution

`resolve_battle_vehicle_critical` joins table selection with control, motive,
engine, temporary weapon-jam and turret consequences in a private world candidate.
Successful results carry the original rolls and occupant notices for the enclosing
attack. Errors publish neither dice consumption nor partial damage. Cargo results
retain the reference's no-effect notice.

Standard/FASA engine hits remove propulsion without setting the advanced
immobility flag. Advanced engine hits immobilize; advanced fusion-engine fuel
hits use that same effect. Combustion fuel explosions remain unsupported and
return an error, as do crew casualties, explosive weapon losses,
ammunition explosions, power-plant explosions and turret blow-off. Callers must
abort an attack on these errors, never reinterpret them as harmless hits.

Tests exercise all three tables, state changes, saved replay, suppression and
unsupported-outcome rollback, including the fusion/combustion distinction.
The full vehicle attack path still needs the remaining consequences and targeting.


### Standard and FASA main-weapon jams

Main-weapon criticals rank every intact mount in hull-face/slot order using one
31-bit draw per weapon, selecting the highest positive value. Existing failures,
recycling and empty ammunition do not exclude a mount. The selected weapon is
marked `disabled` without spending ammunition, changing construction or adding
a timer. Section-local temporary jams continue to exclude every failed mount.

The reference has recycle-dependent recovery: a disabled weapon with no timer
remains unavailable, while one already recycling recovers on its next powered
recycle update. Shutdown pauses that update. Both cases persist across restart;
physical mount/section loss removes the failure. Firing and ammunition controls
reject a disabled mount. The atomic critical dispatcher now handles this outcome.

Tests cover ranking and exact dice advancement, existing failures, no-candidate
suppression, persistent disablement, powered recovery, saved replay and cleanup.
Crew casualties and explosive critical consequences remain unfinished.


### Tactical vehicle crew casualties

Vehicles now use the shared tactical pilot-injury and consciousness path. Injury
counts persist with the vehicle; recovery dice and 30-second retry timers remain
player-owned. Six injuries cause scenario loss, shutdown and cockpit release
without changing armor or construction or killing the player object. Active crew
stun and turret-repair attempts are cancelled. Destroyed vehicles reject cockpit
assignment.

The critical dispatcher applies crew-hit and crew-killed outcomes for a present
assigned tactical pilot and returns the injury report with staged notices. Its
rules explicitly select toughness checks. In-character casualties and absent
pilots remain unsupported: the complete critical candidate, including its roll,
is discarded on error. Those cases need the character/automated-crew lifecycle.

Tests cover injury and recovery replay, scenario death, unchanged material and
player state, timer cancellation, corrupted saves and casualty admission rollback.


### Vehicle ammunition cascade preparation

Vehicle ammunition criticals can now inspect and atomically empty the cascade's
bins within an enclosing attack candidate. The report retains the hit section,
exact ammunition draws and total pending internal damage. All live non-Gauss
bins across the vehicle participate, independent of ammunition mode; plasma
rounds participate here despite being inert in ordinary bin explosions. Gauss
rounds, empty bins and destroyed equipment do not participate. A zero-damage
report requires the critical's weapon-destruction fallback.

This prepares the ammunition consequence only: the caller must resolve internal
damage, resulting criticals, casualties and visibility notices before committing.
The critical dispatcher now consumes this preparation through the atomic
internal-damage resolver described below. Tests cover explicit mixed-bin damage totals, loss
exclusion, unchanged construction, invalid victims and persisted replay.


### Internal vehicle explosion resolution

Internal damage now rolls secondary criticals before reducing structure, including
reinforced/composite structure rounding. Reports retain damage-entry and critical
rolls, nested critical results, ammunition draws, discarded excess and staged
occupant/visibility notices. Vehicle-local internal explosions do not transfer
excess to other faces. Resolution stops further secondary criticals once the
vehicle or hit section is lost and bounds nested cascades to 64 levels.

The critical dispatcher now resolves ammunition cascades, empty-ammunition weapon
loss, ordinary weapon destruction, Gauss explosions and turret blow-off. Explosive
mounts and ammunition are disabled/emptied before secondary criticals, preventing
repeat explosions. Disabled Gauss weapons do not explode. Surviving tactical
pilots take explosion injuries through the shared injury path.

Nested unsupported effects roll back the entire candidate, including spent bins
and all dice. Fatal in-character damage and associated character/evacuation handling remain
unsupported.
The enclosing attack still owns publication of visibility-filtered broadcasts;
these APIs do not make the full vehicle targeting/firing path complete.

Tests cover structure rounding, critical ordering, whole-cascade rollback,
surviving Gauss injuries, disabled-weapon behavior, ammunition overflow remaining
local to the turret, and saved replay.


### Tactical fuel and power-plant explosions

Fuel and power-plant criticals now destroy the victim's sections atomically.
These catastrophes do not apply an area blast to nearby battlefield units. FASA
power-plant explosions with installed CASE destroy only the rear section;
standard power-plant and fuel explosions destroy all remaining sections. Clan
technology and configuration text alone do not install the CASE equipment used
by this check. Advanced fusion fuel hits continue to resolve as engine damage.

Section loss now releases the cockpit when the vehicle is destroyed. Unaffected
sections and their ammunition remain intact after a contained catastrophe; full
explosions clear all section ammunition, weapon timers and turret damage. Tactical
player objects remain in the wreck and are not killed as RPG characters.

The complete candidate is rejected for in-character catastrophes or an
uncontained explosion carrying battlesuits, pending the appropriate casualty
lifecycle. Reports retain section losses and staged occupant/visibility notices.
Tests cover table-specific containment, exact dice advancement, neighboring-unit
isolation, saved replay and casualty rollback. Full attack publication, character
casualties and unsupported unit classes remain unfinished.


### Vehicle hotloaded and incendiary weapon criticals

Vehicle weapon destruction now resolves hotloaded launcher explosions when a
usable normal-ammunition bin remains anywhere on the vehicle. The damage equals
a full salvo. Incendiary ammunition ignites for the weapon's damage only while
the selected weapon is recycling and usable incendiary supply remains. These
checks use live bin contents, individual equipment losses and the live ammunition
selection. Neither explosion spends additional ammunition nor directly injures
the pilot; secondary criticals still apply their own crew effects. Disabled
weapons do not explode. Other firing modes permit ordinary weapon destruction.

The mount is disabled before internal damage and nested criticals, within the
same atomic candidate. Save/reload preserves both eligibility and outcomes.
Complete vehicle attacks remain unfinished.


### Live vehicle firing-mode controls

Vehicles now own persistent firing-mode selections initialized from their
construction templates. Native and Lua heat, hotload, Ultra, rapid-fire, gatling
and rotary controls share the existing equipment rules and require a conscious,
present assigned pilot, running power and an intact, recycled weapon. Hotload
still rejects one-shot launchers. Empty ammunition does not block mode selection.
Repeated rotary selection retains the selected burst; the other controls toggle.

Normal is implicit in saved state. Loading an empty selection never reapplies
non-normal template flags. Snapshot validation shares weapon-family eligibility
with Mechs and rejects explicit normal, unsupported modes and invalid indices.
Lua vehicle state exposes the non-normal `fire_modes` map. Critical explosions
and normal-cycle reservations consult the live mode. Vehicle firing reservations return their effective mode for the enclosing attack.

Tests exercise all six native/Lua controls, notification and state rollback,
saved selections, cockpit guards, snapshot rejection, one-shot restrictions and
live hotload changes affecting critical damage after restart.


### Vehicle mode-aware firing reservations

Vehicle ammunition feeds now share draw ordering with Mechs: use matching live
bins in the mount section first, then canonical section/slot order, skipping lost
and empty bins. Read-only plans expose short supply without consuming ammunition
or dice. Ultra, rapid-fire and rotary reservations draw the requested burst
across bins, falling back to one normal shot and saving that selection when
supply is insufficient. Heat and hotloaded cycles retain their selected modes.

Gatling preparation consumes one saved d6 before attack dice, caps its damage by
available ammunition triples, and preserves the one-point floor when fewer than
three rounds remain. Reservations return effective firing mode and gatling damage
alongside actual ammunition draws. Ground vehicles do not accumulate live heat.
Rejected reservations preserve supply, mode and dice; failed Streak locks still
recycle without consuming ammunition. The enclosing attack must apply targeting,
hit resolution, mode-specific failures and damage before publishing the cycle.
Complete vehicle attacks remain unfinished.

Tests cover all burst sizes, local-bin priority, cross-bin draws, saved fallback,
gatling supply boundaries and exact dice progression, heat without ammunition,
rejected cycles and persistence replay.


### Vehicle armor-to-structure resolution

`resolve_battle_vehicle_armor_damage` applies an already located hit inside one
candidate: hardened armor rounds damage up to half, armor absorbs protection,
through-armor/AP criticals resolve, then any overflow reaches internal structure.
AP checks require no penetration and less than 50 percent of original armor
remaining, with the existing autocannon-size roll penalty. A hit-table critical
takes precedence over the AP adjustment. Combat-safe hits consume only their
entry roll; zero damage is a no-op after input validation.

Penetration reuses the internal-damage resolver without another damage-entry roll
or duplicate hit notice. Its internal critical roll is always consumed, but
additional critical dispatch is suppressed when the armor stage already produced
critical counts. Reinforced/composite structure, nested explosions, crew effects,
section loss and tactical destruction remain part of the same candidate. Excess
damage remains local to the vehicle section. Unsupported casualties roll back
armor, internal damage, equipment, crew changes and random streams together.

The caller still owns hit-table motive effects, searchlights, armor warnings,
weapon-specific damage adjustments and visibility/attack publication. This is a
material-resolution stage, not yet a complete vehicle attack command. Tests cover
material rounding, armor-only and penetrating dice order, critical suppression,
AP thresholds, tactical turret/hull loss, combat safety and casualty rollback.


### Atomic vehicle hit-table impacts

`resolve_battle_vehicle_impact` resolves one positive, already successful damage
group from its incoming arc. Standard, FASA and advanced hit tables now feed the
same armor/internal resolver. The candidate includes hit-routing and conditional
dice, motive speed loss, steering penalties, turret locking, protection loss,
nested criticals and feedback. Any unsupported later consequence discards all
of those changes. Combat-safe routing preserves material state and consumes the
routing and damage-entry rolls without selecting a location or motive effect.

The advanced ground table covers every arc, including cross-face hits and its
specific turretless fallback. Armor-gated roll-three motive checks apply the
tracked/wheeled/hover modifiers, steering penalties and speed loss or immobility;
moving vehicles stage the associated wobble broadcasts. Standard/FASA critical-
proof routing sends roll twelve to the hull, even with an intact turret. Advanced
routing takes precedence over critical-proof table selection; immunity still
suppresses its gated motive check and component consequences.

Saved replay preserves the initial routing roll and the additional delegated
FASA/advanced/critical-proof table roll. Tests cover every advanced table row,
turret survival, movement-class motive thresholds, immunity, combined steering
and penetration, FASA motive/turret effects, exact dice order and full rollback.
Target acquisition, complete firing commands, dug-in vehicle state, searchlight
hits, armor warnings and character casualties remain outside this stage.


### Vehicle terrain and optical targeting geometry

Terrain sight lines now sample either Mechs or ground vehicles through one shared
endpoint model. Mobile vehicles use a half-level eye height; stationary units
retain a one-and-a-half-level height. Signed terrain elevation, hover water/ice
support and saved under-bridge position determine the base height. Existing Mech
posture, flight precision, terrain-break height and target-woods behavior remain
in the shared trace. Coordinate sight queries also use this endpoint model.

Explicit visual and light-amplification queries support mixed Mech/vehicle pairs
without acquiring contacts or changing dice. Map light, weather, sensor-disable
bits, sight obstruction and target submersion retain their ordinary effects.
Existing Mech searchlights and nearby infernos can illuminate vehicle targets;
visual and light-amplification queries reflect that lighting immediately and
after restart. Vehicle infrared signatures still return an explicit unsupported
error. Vehicle-owned sensor selection, durable contacts, target locks and full
firing remain unfinished.

Tests cover ridge occlusion at different unit heights, shallow-water separation,
hover surface height, under-bridge replay, mixed optical queries, searchlight
illumination, read-only state and cross-map rejection.


### Persistent vehicle sensor selection

Vehicles now own active sensor pairs and ten-second pending switches, exposed
through the native `sensor` command, Lua selection API and vehicle state. Current
modes remain active until completion; selecting the active pair preserves an
existing request. Daylight rejects new amplification requests and converts active
amplification back to visual. The shared countdown helper discards expired
requests when power is off, the hull is destroyed or required hardware is lost.

Vehicle radar selection requires AntiAircraft technology. Each vehicle probe
occupies one system slot, including Bloodhound; multiple same-family probes stay
available while any matching installation survives. Individual or whole-section
loss returns affected active slots to visual. Pending probe requests recheck
hardware at completion. Snapshots validate equipment and timer bounds. Sensor
selection does not itself enable the still-unimplemented vehicle IR, seismic,
EM, radar and active-probe queries or durable contact acquisition.

The idle server notices pending vehicle sensor work even after shutdown and keeps
the timer within the ordinary save/rollback boundary. Tests cover native/Lua
agreement, script rollback, saved countdowns, daylight changes, redundant probe
loss, unavailable pending hardware, radar admission, invalid snapshots and failed
server-save retries while powered off. Vehicle contacts, target locks and full
attacks remain unfinished.

### Vehicle optical acquisition rolls

The existing single-attempt and paired acquisition actions support vehicle
observers and mixed Mech/vehicle Visual and Light Amplification targets. Vehicle
hull weights (front 100, side 80, rear 50) and original-turret arc bonus (+15)
feed the shared probability formula. Vehicle direction uses rounded bearing and
integer heading. The paired action validates both sensor queries before rolling,
skips duplicate/unneeded secondary attempts, and commits only observer dice.
Tests cover all hull/turret combinations, both target classes, exact stream
replay through SQLite, unchanged target dice, unsupported secondary rollback,
half-probability secondary attempts, disabled queries and automatic near contacts.
This is an acquisition primitive; durable vehicle contacts and scan cadence are
still pending.

### Durable vehicle contact ownership

Vehicle snapshots own their last acquired contacts. The shared optical-contact
action supports mixed construction classes, keeps acquisition dice atomic, avoids
rerolling retained contacts, supports refresh without acquisition, and removes
ineligible contacts. Shared world validation rejects self/missing targets, empty
sensor roles and contacts across maps or involving unplaced units. Placement and
removal clear outgoing/incoming observations for both classes, and database purge
removes deleted references and contacts from deleted maps. Integration tests cover
acquire/retain/loss, unchanged dice on retention/loss, SQLite replay, mixed-class
placement/removal, database object purge and malformed contact snapshots. Vehicle
contact display, automatic scans and target lock integration remain unfinished.

### Vehicle scenario signatures and scanner perception

The shared trusted signature action accepts live vehicle Things, preserving saved
team, hidden and scenario illumination fields. Vehicle illumination feeds the
existing mixed optical query path. Perception is captured from the assigned
player at startup completion, with the same missing-player default as Mechs;
other running/shutdown ticks and aborted startups preserve the cached value.
Vehicle Lua state exposes detached signature and perception values. Integration
tests cover completion-time skill changes, retention, abort/restart, SQLite
replay, Lua detachment, live-object admission and scenario illumination effects
on Visual and Light Amplification. Automatic scans, contact display and player
hiding controls remain pending.

### Automatic mixed-unit optical scanning

The shared scanner now schedules running tactical vehicles alongside Mechs, with
stable cross-class observer/target ordering and duplicate-observer suppression.
New startups enter scanning on the next tick. Visual/Light Amplification use saved
signature/perception facts and durable acquisition/retention/loss. Unsupported
mixed sensor modes are excluded from that pair rather than failing the heartbeat;
existing Mech-only sensor queries remain available. Acquisition/loss notifications
use identification, friendly labels, vehicle side arcs and saved `brief` settings.
Native/Lua `brief` supports vehicle occupants, retains transactional rollback and
validates snapshot settings. Other cockpit operations retain their existing
construction admission checks. Vehicle labels share the base-36 formatter with
Mechs. Vehicle notifications currently suppress shutdown targets and have no
administrator observer override.

Integration checks cover all mixed contact directions, retained contacts without
rerolls, loss after sensor disable, notification color/friendly filtering, native
and Lua brief changes/rollback, startup/character/unsupported-mode admission,
SQLite replay and an injected server save failure that prevents partial Mech or
vehicle contact commits and succeeds on retry. Contact displays, detailed scans,
vehicle non-optical query rules and weapon target locks remain pending.

### Mixed vehicle contact displays

Native and Lua visible/filtered contact queries now accept vehicle observers and
targets. Shared sensor/display facts preserve same-map and current-visibility
checks, saved identification/team behavior, signed elevation, movement heading,
slot ordering, dead-last native ordering and selected/category filter precedence.
Compact rows use T/W/H/U for TRACKED/WHEELED/HOVER/Unknown movement; verbose rows
retain the reference wording and use vehicle side arcs. Vehicle status columns
currently cover destruction, illumination and startup/shutdown. Vehicle `brief`
selects compact or verbose output; default vehicle lists omit buildings pending
structure-query support. Detailed scans, weapon locks and command networks reject
unsupported vehicle operations without indexing Mech-only state. Mech targeted
radio can render a visible vehicle recipient.

Tests cover all four vehicle movement labels, both observer classes, native/Lua
rows, brief modes, restart replay, category filtering, destroyed/shutdown status,
fresh visibility after map sensor disable, unchanged saved observations/dice and
safe handling by adjacent scan/target/radio consumers. The Jeep asset has a
multi-token ammunition mode syntax not yet accepted by the template parser;
wheeled display coverage uses a valid scenario construction instead.

### Ammunition mode words and abbreviated system entries

Shared template parsing accepts ammunition mode lists separated by whitespace,
pipes or both, preserving flags for the existing loadout validator. Final numeric
brands remain bounded to u8; ammunition may end in a dash placeholder. Known
system entries can omit data and modes, with omitted data represented by the
ordinary dash sentinel. Explicit data is retained for semantic validation.
Weapon rows retain their explicit field layout. Tests cover equivalent flag
spellings, numeric brand errors, abbreviated systems, actual Jeep/HTracked_APC
half-ton hotloaded bins, Huey/Huitzilopochtli ECM construction, snapshot replay
and Jeep contact presentation. No game asset was changed.

A full audit of 235 bundled Vehicle assets with Track/Wheel/Hover/None movement
now reports 234 decoded definitions, 233 resolved loadouts and 232 resolved mass
reports. Outstanding cases: J-27_Transport (rotor anatomy), Shamash (engine weight
rating 58), Svantovit-Streak (Streak SRM-4 ammunition capacity). This supersedes the
Jeep parser gap recorded in the earlier contact-display milestone.

### Complete bundled ground-vehicle construction audit

All 235 bundled Vehicle assets with Track/Wheel/Hover/None movement now decode,
resolve equipment/mass and construct owned Rust vehicle state. The audit example
reports actual construction success separately from simulation support. This
supersedes the three construction failures recorded by the preceding audit.

Hovercraft may use the one-fifth-tonnage engine minimum when their post-suspension
rating is absent from the mass catalogue; the missing entry remains visible in
the engine report. Tracked/wheeled missing ratings remain rejected. Tests cover
Shamash's rating 58, its exact fixed-point minimum, engine technology variants,
snapshot replay and zero-propulsion hover/stationary distinctions. The copied
Svantovit-Streak asset now uses 25 rounds (the reference loader's full-bin clamp),
and the copied tracked J-27_Transport no longer includes an invalid rotor section.
Both have focused construction regressions. No reference-tree files were modified.

### Mixed-class detailed unit scans

The unit scan/report paths support vehicle observers and targets through the
shared sensor/display facts. Vehicle scanner ranges use the same computer quality
and independent template overrides as Mechs, without applying Mech sensor-slot
damage to vehicle gunnery criticals. Ordinary vehicle reports show front/internal
condition bands, surviving weapon mounts and ready/recycling/nonfunctional
status. Trusted Mech observer mode provides exact vehicle armor/internal values;
ordinary pilots cannot request that mode through scan options. Unit summaries
preserve class, movement, placement and current facing. Vehicle heat currently
reports zero because live vehicle heat is not implemented.

Native labels and explicit dbrefs resolve mixed classes. Lua state exposes
vehicle sensor ranges, and native/Lua scans share cockpit warnings with the same
transaction boundary. Tests cover Mech-to-vehicle, vehicle-to-Mech and
vehicle-to-vehicle scans, ordinary/exact disclosure, disabled weapons, options,
pilot admission, configured range versus report behavior, inoperational ranges,
Lua rollback and restart replay. Coordinate/structure scanning and vehicle lock
selection remain pending. Existing Mech scan behavior retains its anatomy and
observer status-report path.

### Mixed occupants in Mech coordinate scans

Mech coordinate scan/report selection includes vehicles in saved battlefield-slot
order. It skips the scanner, other coordinates/maps and occupants without a
currently visible acquired contact. The shared Mech-only map membership API is
unchanged, keeping consumers that require Mech state outside this extension.
Tests reorder a vehicle ahead of a lower-ID Mech, then remove contacts to verify
fallback and empty-hex nondisclosure. Native scan/report, Lua scan, persistence
replay, unchanged state/dice and aborted-script warning rollback are covered.
Vehicle coordinate/structure scanner admission remains pending.

### Vehicle coordinate and structure scanner admission

Vehicle observers can use explicit coordinate unit scans/reports, structure scans
and combined building/mine scans. Cockpit and range checks are shared with unit
inspection. Terrain visibility reads common scanner facts; infrared terrain
queries need no target heat signature. Electromagnetic vehicle terrain queries
fail explicitly, including when selected as the secondary sensor, rather than
indexing Mech signal state. Vehicle default target selection remains pending.

Hidden-building perception reads the vehicle's startup-captured target and owns
its saved dice. Mine recognition draws from the same vehicle stream for the 2..9
range gate, followed by eligible in-character perception and XP. Tests cover
native/Lua coordinate and building outputs, combined terrain output, no-contact
terrain queries, pilot/range/visibility rejection, infrared and unsupported EM,
no-roll concealed structures outside in-character play, range-only mine rolls,
failed/successful perception, experience, restart replay and complete Lua rollback.
This supersedes the vehicle coordinate/structure admission gaps above.

### Vehicle building-contact display

Building contacts admit running vehicle cockpit passengers and render terrain
range, hull arc, sensor roles, integrity and identification status from shared
scanner facts. Native default brief mode includes buildings; C2 excludes them
unless explicitly requested. Saved player building preferences remain independent
and contacts + resolves FollowBrief against the current vehicle setting.

Tests cover native and detached Lua rows, passenger/outside admission, default,
explicit and saved preferences, restart replay, denied/hidden/invisible structures,
silent lock invocation, callback-induced visibility loss, skipped locks for unseen
buildings, and rollback of callback changes/notices. This supersedes the earlier
vehicle structure-contact display restriction. Other callers of Mech-only cockpit
admission retain their construction guard.

### Owned vehicle target selections and lifecycle

Vehicles save one unit or coordinate selection with the shared eight-second
countdown. Explicit native/Lua selection, reselection, clearing, detached state,
selected contact highlighting and default scan/report dispatch use vehicle state.
Vehicles may select acquired Mech or vehicle targets; Mech-to-vehicle locks remain
guarded pending attack support. Coordinate modes include unit-at-hex, buildings,
combined hex inspection, ignition and clearing. Selection does not enable firing.

Lifecycle hooks clear locks on lost contacts, placement/removal, power shutdown,
hull/crew destruction, completed sensor changes and daylight/probe fallbacks.
World validation and repair cover the new references and countdown invariants.
Tests cover both target classes, eight-second replay and completion, native/Lua
selection and abort rollback, selected scans/reports, all coordinate modes,
visibility loss with lock-loss events, sensor/daylight changes, placement of both
classes, shutdown, repair, malformed references/timers/power/coordinates and an
idle vehicle server tick whose failed save must preserve the pending countdown.

### Conventional vehicle weapon aim reports

Rust aim_modifiers and pilot_aim_modifiers route vehicle shooters through a domain
calculation that shares range, target movement, sensor and selected-lock rules.
Reports distinguish vehicle control_damage from Mech sensor penalties and include
stabilizer-adjusted movement, ammunition accuracy, targeting-computer critical
state and applicable target-side Mech concealment/TAG/homing terms. Inspection
uses a cloned vehicle dice stream and does not establish permission to fire.

Tests cover Mech/vehicle targets, supplied and pilot-derived gunnery, settled lock
penalties, vehicle sensor/control hits, damaged stabilizers, restart equality,
unacquired and disabled visibility, unsupported EM, invalid weapon indices,
armor-piercing accuracy, targeting-computer destruction, unchanged saved state and
inspection without matching ammunition. Vehicle live heat, beacon ownership,
network/indirect support and the full firing action remain unfinished.

### Vehicle direct-shot admission and teammate protection

Vehicles own the FFSafety preference with native listing/toggle/explicit controls,
Lua setters and detached state, assigned-pilot admission, persistence and rollback.
The direct tactical check combines current control, target life/map/water state,
readiness, arcs, acquired sensor eligibility, stable stealth locks, Stinger target
restrictions and friendly-fire policy. The query leaves all state unchanged and
does not resolve attacks. Full firing expenditure, effects and publication remain
unfinished; character, underwater, self-targeted, AMS and artillery paths are not
authorized by this query.

Tests cover both target classes, pilot/index/self errors, crew stun, arc override,
missing/disabled contacts, disabled weapons, character and underwater rejection,
empty ammunition, unchanged dice/state, all combinations of teammate preference,
map restriction and target team, native/Lua preference behavior, restart and
aborted-script rollback. The query reuses the vehicle aim report rather than
introducing a second numeric firing calculation.

### Vehicle weapon inspection surfaces

Native weapons, Rust weapon_status and Lua unit.weapons support vehicle mounts
using a generic inspection record with the appropriate section type and shared
row formatting. Optional vehicle failure details preserve physical integrity in
readiness while identifying disabled, jammed or shorted equipment. Mech records
omit absent failures and retain their section representation and booster lines.

Tests cover ordinary readiness, recycle, disabled/jammed/shorted failures, physical
critical loss, armor-piercing selection without supply, stable indices, native/Lua
agreement, detached mutation isolation, unchanged saved state and restart replay.
Full vehicle attack resolution remains pending.

### Atomic vehicle launch phase

Vehicle launch requests combine admitted target-independent aim inputs with owned
attack dice, reservation and recycle state. Gatling preparation runs before attack
rolls; Streak failures retain supply while recycling; confusion permits missed
launches. Conventional misses/out-of-range attempts spend a salvo, glancing follows
the shared policy, and beacon pods exclude glancing. Normal/heat/gatling/Ultra/rotary/hotload/rapid and
supply-fallback normal cycles are supported. Ultra loader loss on a two destroys
the mount without ammunition expenditure or recycling. Rapid and caseless misloads resolve shooter-local internal damage and secondary
criticals before spending surviving reserved supply. Unsupported character
casualties discard the entire launch candidate. Rotary and hotload feed jams persist separately from critical
failures, block readiness, and survive ticks and reload. Timed vehicle clearing
is connected through the existing unjam command, Lua action and server transaction. Target effects and
host publication remain caller-owned and are not yet connected to vehicle fire.

Tests cover hit/miss/glancing boundaries, out-of-range expenditure, full restart
replay, exact next-die ordering, failed and confused Streak locks, supply-limited
gatling preparation, burst fallback, loader rejection and rollback for invalid
range, target arithmetic, pilot admission and repeated unready launches.

Ultra launch tests cover every attack total from two through twelve, next-die
ordering, saved-state replay, physical loader loss, unchanged adjacent mounts,
normal two-round expenditure and atomic rejection of repeated firing attempts.

Feed-jam tests exercise every attack total for hotload and each rotary burst,
checking exact thresholds, ammunition, dice, intact mounts, restart and timer
persistence, invalid saved indices and cleanup on equipment destruction.

Vehicle unjam tests cover success/failure skill rolls, saved countdown replay,
empty supply and silent expiry, native/Lua admission and rollback, character XP
publication rollback, visible-only ejection broadcasts, and idle server retries
after rejected vehicle persistence updates.

Misload tests cover rapid feed failures, caseless ignition precedence, exact
attack/propellant/internal-damage dice ordering, short-supply fallback on failed
launches, safe and enabled critical policies, restart replay, lost bins, surviving
supply after fatal hull damage, and rollback of unsupported character casualties.

Vehicle grouped target damage now uses the same weapon-packet engine as Mech and
terrain attacks. Tests compare complete impact sequences and saved dice for direct
fire, missiles, LBX clusters, Ultra and rotary bursts; cover missile interception,
glancing gatling/direct hits and Streak confusion; and verify early termination on
hull loss plus atomic rejection of unsupported target effects and invalid inputs.
Native firing publication, character casualties and dedicated heat/inferno/beacon/
plasma vehicle effects remain unfinished.

Shared-unit consolidation: Mech and vehicle launch adapters now call one attack
and propellant outcome function, one supply-fallback rule, and one gatling roll
rule. Mech and vehicle feed clearing use a single admission/countdown/skill/XP/
publication workflow with small state adapters. Existing tests exercise both
callers, preserving their distinct anatomy and inventory behavior.

Atomic vehicle shots now combine shared admission and sensor dice with launch and
existing target-type damage. Integration tests cover hits and misses on Mechs and
vehicles, database replay, repeated-fire/admission rollback, failed Streak locks,
Mech AMS expenditure on hits and misses using shooter dice, and Angel disturbance
from existing emitters. Native/Lua fire publication and the dedicated unsupported
vehicle effects remain outstanding.


### Shared firing publication for vehicles

Native `fire` and Lua `btech.unit.fire` now dispatch tactical vehicle shots through
the existing configured firing transaction, including Mech target consequence
publication and rollback of state, dice and staged output. Common formatting serves
both classes; mixed observer audiences use one scanner visibility policy. Regression
coverage compares native/Lua hits and misses against both target classes, aborts a
callback after firing, and rejects recycling weapons without partial changes.
Vehicle coordinate shots and the previously listed dedicated effects remain guarded.


### Vehicle occupied-hex firing and common membership

Vehicle unit-at-hex locks now fire at the first non-removing other occupant in saved
map slot order. Both construction classes use the same membership/hex enumeration
for scanning and firing. Coordinate cockpit and observer formatting is shared with
Mech fire and includes mixed observers. Native/Lua tests cover both target classes,
explicit target precedence, unchanged locks, hidden/character target rejection,
empty and terrain guards, pending-removal ordering and complete failure rollback.
Dedicated empty-hex and terrain effects remain incomplete for vehicles.


### Vehicle automatic missile defense

The existing native/Lua AMS controls and saved switch now support vehicles. Both
classes share mount/bin ordering, capability loss, attack-owned defense dice,
supply/interception caps and feedback. Vehicle missile targets use the same defense
phase before shared salvo packet resolution. Tests cover all four AMS identities,
hits and misses, supply shortage, restart replay, power/switch/recycle/critical and
bin selection, control authority, native/Lua parity and callback rollback.


### Shared beacon and thermal shot effects

One resolver now applies Mech target pod attachment, coolant and direct flamer heat
for both launcher classes. Vehicle shots expose the same pod/thermal reports and
use common cockpit feedback; pod interception retains the one-pod defense report
without duplicate AMS messages. Narc/explosive/iNarc controls share authority,
selection and storage across classes. Tests cover standard/Clan Narc and iNarc
homing/haywire/ECM, hits, misses, defenses, persisted replay, coolant/flamer host
parity and rollback, mode controls, and Clan plasma damage against both classes.
Vehicle target pod/thermal lifecycle and vehicle coolant self-application remain
explicitly guarded.


### Vehicle target beacon lifecycle

Vehicle shots attach standard Narc and iNarc homing/haywire/ECM to vehicle targets,
with shared interception and pod feedback. Hit routing is separated from armor
damage and reused for pods, retaining table-specific motive/turret effects. Saved
vehicle beacons participate in aim, missile guidance and electronic interference;
section destruction and snapshot validation enforce surviving-section ownership.
Tests cover hits/misses/defenses, replay, native/Lua rollback, live guidance/aim/ECM
changes and section-loss cleanup. Vehicle pod-removal physical controls and live
heat remain incomplete.


### Vehicle pod inspection and crew removal

Shared pod rows and table formatting include vehicle anatomy. Native `removepods`
and Lua `btech.unit.removepods` start the vehicle-specific 60-second crew action,
with ordinary Narc retained. Start guards, movement/fire exclusion, shutdown/death
expiry, snapshot validation and restart replay are covered. A server test forces
expiry persistence to fail, checks that the old timer and effects remain saved,
and verifies the retry removes only iNarc. Command access and help coverage include
the new route. Vehicle live heat and broader combat integration remain incomplete.

### Shared target-side aim across unit classes

Mech aim now accepts vehicle targets through the existing mixed-unit geometry and
sensor queries. Both shooter classes use one calculation for target movement,
friendly TAG assistance, homing-pod accuracy and concealment. Shooter equipment,
heat and mount damage remain construction-specific. Regression coverage compares
both shooters against moving, reversing and powered-down vehicle targets with and
without homing pods, checks read-only dice ownership, and repeats aim after saving
and loading. This enables aim inspection; Mech-to-vehicle target-lock and firing
admission still require integration with vehicle damage and publication.


### Mech-to-vehicle tactical firing and shared target reports

Conventional Mech fire now reaches the existing vehicle salvo and beacon resolvers.
Both shooters share target hit geometry, guidance, damage feedback, broadcasts,
attachment routing and configured vehicle hit policy. `BattleTargetSalvo` describes
either recipient anatomy; Mech and vehicle Lua shot results share the same
`salvo.kind` / `salvo.report` shape. Mixed target locks use one saved-state validator.

Tests cover Mech-to-vehicle hits/misses, lock settling across restart, native/Lua
state parity and abort rollback, occupied-hex selection, placement cleanup, crew
and character guards, unsupported burning rejection, SRM packet counts after AMS,
and Narc/iNarc attachment with interception. Existing Mech combat tests use the
shared report shape while retaining their damage, experience and publication
assertions. Character vehicle combat, underwater firing and vehicle heat remain
incomplete; this does not mark the broader integration complete.

### Empty tactical crew injuries and recovery

Both Mechs and vehicles own a saved recovery component for an unoccupied cockpit.
It shares the player recovery target table, toughness roll, initial-check and
thirty-second retry implementation. Crew criticals now accumulate tactical injuries
without requiring or inventing a player. Further injury updates the target without
postponing an existing retry; fatal injury or material destruction clears recovery.
Mech tactical impact injuries use this path for empty recipients as well.

Taking a cockpit transfers its recovery component to the assigned player, preserving
the countdown and private dice. Unconscious empty crews receive the same target
immobility modifier. The normal heartbeat advances pending recovery while powered
down, inside the existing save/notice transaction. Lua exposes only
`crew_recovery_remaining`, keeping the random stream private.

Regression tests cover both classes, injury during recovery, saved replay, cockpit
transfer, fatal cleanup and invalid snapshots. A powered-down empty-vehicle server
test injects a save failure and verifies countdown and dice remain unchanged until
a successful retry. This resolves the previously documented empty-vehicle crew-hit
rejection; character vehicle casualties and the broader integration remain pending.

### IS plasma against ground vehicles

IS plasma-rifle hits now use the shared vehicle packet/impact resolver for both
Mech and vehicle shooters. The executable reference hit path uses catalogue damage
(ten, or five for a glancing hit); the post-damage plasma heat hook applies only
to Mech targets. The comment mentioning extra non-Mech plasma damage does not have
a corresponding branch in that hit path. No vehicle heat state or additional
thermal dice are introduced by these shots.

Both shooter admission paths and direct vehicle salvos now share one thermal
recipient check. Coolant, flamer heat mode and inferno still require their vehicle
heat/burning lifecycle. Regression tests compare vehicle state and dice against
an ordinary impact, cover misses/glancing/restart, and verify native/Lua parity and
callback rollback. Existing Mech plasma heating remains covered separately.

Thermal follow-up boundary: `unit/mech_heat_state.c::mech_uses_heat` excludes
ground vehicles from the Mech heat update. Direct flamer heat-mode and coolant
hits update weapon heat in `combat/mech_hit_resolution.c`; inferno instead goes
through `combat/mech_combat_misc.c::mech_heat_effect_apply`, selecting vehicle
explosion or advanced section-fire behavior. These are distinct paths; completing
vehicle thermal effects should not add Mech overheating rules to ground vehicles.


### Ground-vehicle weapon heat and direct coolant/flamer effects

Ground vehicles persist a finite `weapon_heat` balance, including negative coolant
credit. Firing and AMS add heat; coolant subtracts its catalogue damage, and flamer
heat mode adds its catalogue damage instead of material damage. The same direct
effect resolver handles Mech and vehicle carriers and recipients. Firing modes
share one launch-heat calculation, including burst fallback, gatling damage, and
failed Streak launches. Ground vehicles do not run Mech heat sampling or overheating.

Coolant heat mode redirects vehicle shots to their carrier before target selection,
just as it does for Mechs. It needs neither a contact nor a unit/hex lock; readiness,
crew authority, expenditure, attack dice, and transaction rollback still apply.
Native and Lua firing use the same path. Lua vehicle inspection exposes
`weapon_heat`; the vehicle launch expenditure reports the heat already applied.
Snapshots require the new field rather than silently supplying missing state.

Regression coverage exercises both shooter classes, hits and misses, ordinary and
heavy vehicle flamers, negative coolant credit, saved replay, native/Lua parity,
callback rollback, self-cooling with no selection or an existing hex selection,
empty feeds, recycle rejection, AMS heat, and launch failure/gatling heat. Thermal
updates leave ground-vehicle heat balances unchanged. Inferno remains guarded:
vehicle burning is a separate lifecycle, not Mech overheating.


### Vehicle inferno, section fires and crew extinguishing

Inferno ammunition selection now uses the same recycled-weapon and disposable-
launcher checks for Mechs and vehicles. Both shooter classes route surviving
inferno missiles to the vehicle outcome resolver. Missile cluster counting,
interception caps and remaining packet sizing are shared with Mech target salvos;
inferno exposure wording and duration calculation are shared as well.

The configured `fasaadvvhlfire` policy selects mobile-vehicle consequences. With
standard rules, a target-owned 2d6 roll above eight destroys the vehicle through
the existing tactical explosion handler. Advanced rules ignite each surviving,
not-already-burning section, applying 1d6 damage immediately through the existing
armor/penetration/critical resolver and scheduling its next pulse in 60 seconds.
A pulse rolls before checking section survival; surviving fires repeat unless the
roll is one. Repeated inferno hits do not reset active section timers. Stationary
units instead accumulate three minutes of jelly per rounded-up missile pair,
without immediate damage or weapon heat. Jelly expiry and hex illumination include
stationary vehicles. Vehicle immersion does not use the Mech steam-extinguishing rule.

`extinguish` and `btech.unit.extinguish(unit, pilot)` start the same saved 120-second
crew action while the vehicle is not running. The start requires an assigned,
conscious operator and an existing section fire. Existing attempts continue after
startup; section pulses continue after shutdown. Burning, jelly, and extinguishing
countdowns independently activate the server heartbeat. Going objects pause.
Snapshots require and validate all three fields; Lua exposes detached countdowns.
Fire damage, timer advancement, random state and notices remain in the same server
save transaction, including failed-save retries. Unsupported character casualties
still reject the entire candidate rather than partially applying fatal effects.

Tests exercise both carriers, hits/misses, AMS, native/Lua ammunition selection and
firing, callback rollback, standard explosions, stationary jelly, repeated hits,
section pulse dice ordering, spontaneous burnout, destroyed sections, shutdown,
crew extinguishing, destruction cancellation, restart replay, invalid snapshots, rejected fatal character
ticks and a real server save failure. Vehicle terrain-fire exposure and character
casualty integration remain separate gaps; autopilot and repair are excluded.

Vehicle destruction cancels section fires, stationary jelly and crew extinguishing
together; a powered burning vehicle shows `B` in visible contact status.


### Vehicle movement through terrain fire

Ground movement admits burning terrain and fire overlays. Under `fasaadvvhlfire`,
each newly crossed burning hex performs the target-owned advanced fire check;
remaining within the same hex does not repeat it. Standard movement admits the
terrain without that optional check. Wheeled and hover modifiers are shared with
advanced motive hits. Outcomes are no effect, a motive check, an immediate section
sweep, or persistent section ignition. Motive arithmetic, damage application,
feedback and ignition reuse combat/inferno handlers rather than duplicating them.
The section sweep retains all eight diagnostic damage draws, including unused or
destroyed section slots, while live sections enter the shared armor-damage path.

`BattleMovementRules.fall.vehicle_impact` carries the configured vehicle policy into
movement. The server uses the same configuration adapter as firing. Traced fire
positions remain on the movement segment; disabling damage stops at the affected
crossing. Damage and movement use the enclosing candidate, so rejected casualties
roll back position, throttle, armor, timers and dice. Other unresolved vehicle
hazards retain their existing admission stops.

Regression coverage checks every fire threshold for tracked, wheeled and hover
vehicles, shared motive outcomes, exact damage/dice order, ignition reuse,
configuration-off behavior, multiple crossed fire hexes, same-hex movement,
stopping on a disabling crossing, persisted replay and fatal-casualty rollback.

### Live vehicle mass and mine selection

`BattleVehicle::mass` and template mass use one calculation over current or intact
material. Armor, structure, section equipment and loaded ammunition contribute
current weight; damaged mounts retain mass until their section is gone. Crew loss
does not remove the engine or cockpit. Complete loss of structure removes those
components. Mechs and vehicles share proportional structure accounting with wide
intermediate arithmetic. Lua `btech.unit.state` exposes detached vehicle mass.

The read-only `mine_activations` query now accepts ground vehicles and uses the
same ordered fields, coverage, physical-event rules and weight thresholds as
Mechs. Hovercraft above submerged mines are excluded by the existing altitude
rule. Tests cover ammunition crossing a whole-ton trigger threshold, material
loss, high authored internal values, dry/water/ice heights, no query mutation,
SQLite restart and detached Lua results. Vehicle movement uses these queries through the shared mine event path. Mixed
blast damage and movement publication are described below.


### Mixed mine blasts and vehicle heat

The mine blast traversal now uses the shared mixed map-slot order. Geometry,
five-point packets, neighboring blasts, forest ignition and colocated field
removal remain common. `BattleBlastImpact` identifies each packet as Mech or
vehicle damage. Vehicle packets use the existing hit tables, motive effects,
armor and critical cascades, with observer audiences captured before damage.
Vehicle activation and map-wide command detonation use the same event and field
selection paths as Mechs. Vehicle packets continue after fatal damage using the shared vehicle follow-up
path described below.

`BattleFallRules.vehicle_impact` carries vehicle policy through chained falls,
mines and terrain effects. Shot, movement, heat, stagger and configured command
boundaries propagate it. Ground movement uses that policy for terrain fires;
there is no second movement-only vehicle policy.

Blast heat differs from missile inferno: stationary vehicles receive six seconds
of jelly per heat point, standard mobile vehicles use the shared heat-explosion
check, and advanced vehicle fire uses the shared terrain-fire check. Missile
inferno retains its direct section ignition and pair-based stationary duration.
Jelly accounting and heat explosions are shared between both heat sources.

Tests cover mixed occupant order and typed packets, command/vibra spread, field
removal, standard/FASA/advanced safe-impact dice, heat explosion thresholds,
stationary duration, advanced motive checks, duration overflow, SQLite replay,
vehicle-trigger selection, vehicle-origin command detonation and rollback when
a later target requires unsupported vehicle character casualties. Vehicle
character casualties remain pending. Movement-triggered events are described below.


### Vehicle movement mine events

Ground vehicles now use the common mine event resolver on every newly entered
hex, with current weight and support height. Unrelated minefields do not prevent
travel, and movement within the starting hex does not trigger an entry event.
Mine damage resolves before terrain fire; immobilized survivors still receive
the entered hex's fire exposure. The vehicle stops at the affected crossing when
disabled. Vehicles disabled by an earlier occupant's blast cannot take their
previously scheduled movement turn.

Movement retains mine reports for the existing host consequence publisher,
including one `on_mech_mine_trigger` callback per selected trigger. Late callback
failure restores positions, armor, minefields, random streams and staged notices.
A later unsupported hazard or map edge no longer suppresses effects along the
already traversable part of the route. Slope speed changes apply at each crossing.

Regression coverage includes multiple mines in one update, field removal and
packet dice, callback counts and rollback, unrelated fields, same-hex motion,
mine/fire ordering, stopping before a later boundary, damage to a later moving
vehicle, saved replay, and a real heartbeat retry after rejected mine deletion.


### Vehicle damage after fatal packets

Mine blasts and weapon salvos now finish their accepted packet sequences after
hull loss. Both use the same vehicle impact, armor and penetration handlers.
Each packet selects a location from the current surviving anatomy. Hits on lost
sections consume their damage-entry roll and are discarded without transferring;
surviving armor and structure can still be damaged. Mine footprints include
existing wrecks. Ordinary direct attack admission still rejects destroyed units.

Destroyed vehicles do not repeat crew critical cascades, while table selection
and damage diagnostics retain their order. The reports retain all packets and
mark the destroyed unit consistently. Tests cover fatal hull hits followed by
lost-face and surviving-turret hits, armor and internal damage on a wreck,
turret loss followed by hull loss under all three vehicle tables, exact next
dice, persisted replay, and the existing movement rollback/retry paths.
Vehicle character casualties remain unfinished.

Every mine blast now performs the mobile vehicle heat response, including zero-heat
ordinary blasts and targets already destroyed by earlier packets. Stationary units
only accumulate the supplied jelly duration. Mobile heat uses the shared advanced
fire check or standard explosion handler; a heat explosion consumes any remaining
wreck sections and cancels its thermal events. Terrain fire still checks the newly
entered hex after a fatal mine, before movement stops.


### Fire damage after hull destruction

An admitted section ignition or fire sweep completes damage to every surviving
section after hull loss, using the same follow-up armor resolver as weapon salvos
and mines. Hull destruction cancels existing thermal events once. Ignition can
then schedule new section fires, including the final pulse for a section destroyed
by its initial burning damage. Surviving wreck sections continue their scheduled
pulses until the section is lost or a one-point pulse extinguishes the fire.

Regression coverage checks fatal ignition and sweep damage, cancellation at the
initial destruction transition, retained wreck timers, dead-section pulse disposal,
and matching post-restart fire damage and random state. Additional coverage checks
zero and positive blast heat on intact and destroyed units, ordinary and Inferno
mine explosions on wrecks, exact movement dice after fatal mine/terrain fire, and
replay of these effects after SQLite reload. Character casualty guards remain.


### Mixed-unit artillery arrivals

Artillery damage cells now visit Mechs and ground vehicles in shared map-slot
order, including surviving wreck sections. Mine and artillery arrivals use one
packet and heat handler (`blast_damage`); `BattleBlastImpact` preserves the
anatomy-specific result for publication. Mechs use the selected weapon, punch,
or kick table, while vehicles always use their configured normal location table.
All packets finish before the shared mobile zero-heat fire/explosion check.

Artillery retains its own geometry, packet sizes, scatter, smoke and mine deposits.
Vehicle height filtering uses the artillery bounds measured from the water surface,
so sufficiently submerged hulls are excluded while surface hovercraft remain
eligible. Visible arrival notices use the common mixed-unit scanner audience.

Tests cover mixed center/neighbor ordering, standard and cluster packet sizes,
vehicle heat explosions, SQLite world/flight replay, late character-admission
rollback, vehicle observer notices and the underwater cutoff. Existing Mech
surface and mine tests exercise the extracted handler. Vehicle character casualties,
explicit mixed observer links are described below.
Rear selection is retained across occupants and handed to the shared armor stage.
Its vehicle behavior is covered below.


### Rear selection across blast occupants

Mines and artillery preserve a cell's rear-hit selector across later occupants,
including front-facing Mechs. Vehicle table routing still records its selected
face; the shared armor handler redirects a selected front face to the rear when
that selector is set. The damage-entry roll precedes the rear-hit diagnostic;
SalvageTech suppresses that diagnostic. Combat-safe handling returns before it,
and lost-section handling follows the remapping and diagnostic stage.

Regression coverage exercises both blast sources, all three vehicle critical
tables, a Mech between the rear-facing and front-facing vehicles, the SalvageTech
exception, final front/rear armor and exact saved next dice after SQLite reload.
Ordinary weapon salvos retain their existing admission and face routing. Actual
towing mechanics remain a separate integration gap.


### Vehicle artillery launching

Native `fire` and Lua `btech.unit.fire` now route vehicle artillery through the
same configured artillery transaction as Mechs. The existing vehicle launch
handler owns reservation, attack dice, hotload jams, inventory, recycle and weapon
heat; artillery owns aim, payload, map queue and feedback for both anatomies.
Queue admission uses the shared unit position view. Shooter-local damage is typed
as `BattleArtilleryMisload`, while expenditure exposes common weapon facts.

Ground vehicles can launch unassisted coordinate artillery, subject to the existing
range, water, arc, underground and configured spotter requirements. Explicit vehicle
observer links and automatic trajectory correction are described below. Ordinary direct-weapon admission still rejects artillery so callers cannot
bypass the configured artillery checks.

Regression tests cover matching native/Lua launches, ammunition/heat/recycle,
reload and delayed arrival, explicit-target and authority rejection, arc rejection,
failed-callback rollback, cluster/smoke/mine payloads, and hotload jam/success with
saved state. Existing Mech artillery tests remain applicable to the shared path.


### Mixed automatic artillery correction

Vehicle snapshots now retain artillery adjustment, exposed through the same state
API and applied by shared artillery aiming. Retargeting clears the vehicle's value.
The existing correction handler visits mixed map slots for automatic friendly
observation, allowing either Mechs or vehicles to observe either firing class.
Selection checks visibility before the separate running-state gate, retaining the
reference behavior where a stopped first visible friendly prevents fallback.

Tests cover both observer classes, friendly/enemy and running/shutdown cases,
arrival across SQLite reload, the adjusted next firing target number, Lua state,
retarget reset, Mech artillery with vehicle observation, and stopped-first ordering.
Explicit spotter links and indirect missile consumers are described below.


### Explicit mixed spotter links

Mechs and vehicles now share self-spotting declaration, friendly acquired observer
selection, role/fire restrictions, recycling checks and live link validation. Vehicle
snapshots retain the selected observer and Lua exposes it. Both shooter classes use
one indirect-aim builder for observer skill, movement, settling and optics, and one
water-layer restriction. Mech spotting experience can use a vehicle observer without
assuming its anatomy. Existing vehicle character-combat guards remain.

Vehicle indirect fire takes the observer's unit target even without a shooter
contact or firing arc. Explicit artillery links take the observer's visible hex.
The shared correction handler honors either observer class and retargeting clears
all same-map dependents in both construction stores. Launch, AMS, packet damage,
queueing and publication continue through their existing shared handlers.

Tests cover all mixed shooter/observer pairs, native/Lua link equality, shooter
contact loss and rear-facing firing, moving vehicle observers, observer lock delay,
SQLite link/fire replay, role invalidation, self-spotting/recycle rejection, failed
callback rollback, selected artillery coordinates and dependent correction reset.
Existing Mech spotting/motion tests continue to exercise the same implementation.


### Mixed electronic emitters and receivers

Ground vehicles now expose saved Guardian/Angel modes and field observations.
Mechs and vehicles share native/Lua controls, ECM/ECCM cancellation, range/team
filtering, field snapshots, heartbeat admission and transition notices. Hardware
adapters retain construction-specific slot requirements. Damage and shutdown
reconcile modes through the same control rule. ECM pod attachment uses one shared
receiver update, including saved observations and ready-light notifications.

Tests exercise vehicle emissions against Mech and vehicle receivers, Angel
countermeasures, repeated-refresh silence, SQLite replay, equipment loss, shutdown,
invalid saved emission rejection, native/Lua equality and failed callback rollback.
Vehicle C3 participation is described below.


### Shared command-computer hardware

Mechs and ground vehicles now derive C3 master, slave and C3i availability from
one inventory calculation. The calculation groups master slots within sections,
keeps installed and working counts separate, and applies the same master-loss
rule to both unit types. Chassis adapters supply slot sizes and damage facts: a
Mech master uses five slots and C3i needs two, while vehicle computers use one.
Destroyed sections and whole-unit destruction disable their computers without
erasing installation facts. Power and membership do not affect hardware counts.

Vehicle Lua state now exposes `c3_hardware`. Tests cover independent vehicle
masters, slave fallback restrictions after master loss, C3i loss, turret and hull
destruction, JSON/SQLite replay, Lua inspection and shipped master/slave assets.
Existing Mech hardware and C3i behavior exercise the same calculation. Mixed network membership, status, messaging and targeting are described below.


### Mixed C3 and C3i networks

One membership engine now handles Mechs and ground vehicles, including independent
family identities, working-master capacity, admission, normalization and snapshot
validation. A borrowed participant adapter supplies hardware, map/team, operator,
motion and protection facts. The shared message/status/contact handlers include
both classes and use their movement markers. Both shooter classes apply the same
network range rules; visibility permission and physical reach remain independent.
Vehicle observations can also provide network coordinate ranges to Mech shooters.

Vehicle state persists both identities and Lua exposes both member lists. Damage,
team changes and map removal clear affected links; shutdown retains membership.
Mixed tests cover family capacity, SQLite replay, hardware loss, native/Lua control
equality and rollback, messages, reports, range sharing and team/map transitions.
Existing Mech network regression tests run through the same implementation.


### Shared vehicle coordinate aim

Vehicle `aim_hex` previews now use the same coordinate-aim handler as Mechs.
Weapon range, ammunition accuracy and inherent weapon accuracy come from one
shared calculation; chassis adapters add movement, damaged controls, heat and
equipment effects. Vehicle unit-target aim reuses its weapon adapter instead of
maintaining a separate coordinate implementation. Terrain modes apply their
existing hex bonus and omit unit movement, locks and optical target terms.
C3/C3i coordinate assistance and the Stinger restriction remain shared.

Tests compare vehicle unit/hex weapon terms, all five target modes, movement and
control damage, mixed-network range, read-only Lua previews, SQLite replay and
Stinger/coordinate rejection. Existing vehicle aim and Mech motion regressions
exercise the refactored common arithmetic. Vehicle terrain launch and effect publication are described below; aim previews
remain separate from firing admission.


### Vehicle coordinate launches and shared terrain effects

Ground vehicles now enter the same coordinate-fire transaction as Mechs for empty
unit-at-hex targets and Hex, Clear, Ignite and Building modes. The common launch
adapter delegates reservation and attack dice to existing chassis handlers and
normalizes their expenditure and tagged `BattleLaunchMisload` reports. Artillery
uses that adapter too. Missile grouping, woodland damage, building packets, surface
checks and host feedback remain single implementations. Shared map and dice access
lets those handlers accept either shooter class. Vehicles do not receive Mech recoil.

Tests cover native/Lua mode parity, ammunition/heat/recycle, SQLite replay, failed
callback rollback, empty coordinates, ice fracture and vehicle misloads. Surface
visibility now honors the reference exception for a coordinate target on zero-depth
ice, without bypassing intervening terrain; both eye heights are tested. Existing
Mech terrain, surface and artillery regressions cover the same code. Vehicle
character combat remains guarded. Mixed occupant consequences are covered separately by surface-failure tests.


### Durable vehicle water destruction

Vehicle snapshots now retain a `flooded` cause independent of hull integrity and
crew injury. Environmental callers can mark an otherwise surviving vehicle as
water-destroyed without fabricating armor or internal damage. Hull loss, crew loss
and flooding share one control cleanup: release the pilot, stop motion, clear
recovery and fire events, shut down power and reconcile electronics/networks.
Repeated destruction does not cancel later wreck effects. Lua inspection exposes
`flooded`, and ordinary destroyed-unit admission prevents restarting the wreck.

Tests cover unchanged material/ammunition/crew facts, control cleanup, later fire
preservation, hull-wreck idempotence, JSON/SQLite replay, Lua state and invalid
running-wreck rejection. Surface failure now connects vehicle fall damage and subsequent water destruction.
Mixed Mech/vehicle tests cover ice damage, hover exclusion, zero-depth ice,
bridge retained heights, SQLite replay and rollback of prior neighbor damage.
Vehicle character casualties, VTOLs and remaining movement hazards are unfinished.


### Shared reverse-slope checks and vehicle fall placement

Mechs, wheeled vehicles and hovercraft share reverse-slope pilot checks, feedback
and successful-check experience handling. Tracked vehicles remain exempt. Vehicle
success skips ordinary step effects and slope speed loss; failure uses the shared
fall calculations and vehicle impacts, restoring the previous position for an
uphill failure. Descent failures remain in the lower hex. Tests cover both vehicle
movement types, success and failures, exact successful dice consumption, disabled
checks and SQLite replay. In-character vehicle fall consequences remain guarded.

Retained bridge height applies only to the departure hex. Movement ignores the
origin when checking new terrain, then compares destination support height with
the retained departure height. A regression verifies the resulting slope speed
loss and clearing of retained height after entry.


### Vehicle cliffs

Tracked, wheeled and hover vehicles use the same speed-based avoidance modifier
as Mechs for cliffs exceeding their one-level limit. Success restores the previous
position and stops controls. Failed uphill checks use signed-speed crash severity
(or zero severity with skid rules), then restore position; failed descents fall
to the destination. Falls retain pilot checks, packet damage and fall mines.
Tracked/wheeled drops into water destroy surviving non-waterproof vehicles.
Ordinary entry mines and fire are bypassed by the cliff event.

Tests cover all three movement types, stop/crash/drop outcomes, zero-damage skid
falls, exact avoidance dice, waterproof protection and SQLite replay. Ground
vehicles are exempt from ordinary stacking collisions. Vehicle auto-fall controls are covered
by the subsequent preference integration.


### Shared auto-fall preference and cliff admission

Vehicles persist AutoFall and expose it through the existing native preference
command and Lua setter/state. Catalog metadata and the cliff avoidance decision
are shared with Mechs. Tests cover native toggle/explicit settings, wrong-pilot
rejection, callback rollback, saved replay, exact skipped downhill avoidance dice,
unchanged uphill avoidance and the pilotless stop exemption.


Ground vehicle travel no longer stops merely because a hex is occupied. Reference
stacking eligibility excludes ground vehicles both as movers and counted occupants.
A regression compares travel through eight mixed stationary occupants with an empty
field, proving identical mover state/dice, unchanged occupants and saved replay.
Explicit physical attacks and terrain collisions remain separate behavior.


### Ground vehicle water entry

Tracked and wheeled vehicles now resolve ordinary water entry using a speed-based
pilot check. Successful avoidance restores the previous position and stops;
failure floods the vehicle without fall damage or crew injury. Hovercraft,
waterproof vehicles and zero-depth water bypass the check. Cliff and reverse-slope
handling take precedence, and water avoidance skips ordinary step mines and fire.
Pilotless units stop without dice. Character flooding remains guarded until its
casualty effects are implemented; rejected entry restores the complete movement
transaction. Tests cover both drivetrains, smoke overlays, exact dice use, unchanged
material/ammunition/crew, successful stops, flooding, exemptions and SQLite replay.


### Hovercraft low-span collision

A hovercraft moving beneath a bridge now checks control before entering an adjacent
one-level span. The same speed modifier used by cliff avoidance applies, including
skid configuration. Failure applies one level of fall damage; success and failure
both restore the previous position/under-span state and stop movement. Entry mines,
ordinary slope slowdown and later traced entries are bypassed. Terrain control
rolls, pilotless exemption and optional XP are shared with other unit types and
terrain hazards. Tests verify both outcomes, skid behavior, exact dice, unchanged
material on avoidance, restored height, and durable replay.


### Shared vehicle obstacle entry

Vehicle forest/rough admission now applies the configured `newterrain` checks
instead of stopping fast travel. A single obstacle profile selects tree/rock
eligibility, difficulty and fall severity for tracked, wheeled and hover units.
The control/XP and fall implementations remain shared. Production movement forwards
`newterrain`, with the shipped disabled default for tracked/wheeled checks and
unconditional hovercraft tree checks. Obstacle checks precede slope slowdown;
ordinary entry mines/fire follow even after a failed obstacle check.

Pure tests cover speed thresholds, reverse speed, heavy-forest modifiers and
configuration eligibility. Mixed scenario tests cover all three movement types,
configuration-off travel, terrain beneath smoke, damage, exact dice ordering,
rollback and persistence.


### Shared vehicle ice fracture

Surface-level tracked/wheeled entry now uses the existing ice probability and
ordered mixed-occupant break transaction. The check borrows each unit's own dice
through the shared adapter and excludes hovercraft/submerged units. Vehicle falls
retain nested ice-break reports and apply outer damage after the new water fall.
Waterproof equipment exempts the triggering vehicle from post-fracture flooding,
while neighboring vehicles retain their independent flood consequence. Movement
forwards neighboring Mech fall reports to the existing character publication path.

Tests cover mixed occupant ordering, both ground drivetrains, trigger and neighbor
waterproof behavior, zero depth, smoke, exact dice, underwater travel, nested falls,
failed-character rollback and persisted replay. Vehicle character casualties remain
unfinished.


### Shared character pilot injuries for vehicles

The character pilot injury action accepts Mechs and ground vehicles through the
same health, consciousness and recovery implementation. Vehicle character status
is saved independently of the six-hit tactical injury counter and exposed in Lua
unit state. Fatal character injury releases the pilot and stops vehicle controls
without inventing armor damage, ammunition loss or flooding. The shared evacuation
action moves non-wizard occupants and retains experience using the configured
rules; failure restores both unit state and occupant locations.

Tests cover zero-hit behavior, injuries beyond the tactical limit, unconscious
control rejection, fatal cleanup, wizard exemption, failed evacuation rollback,
invalid active wreck snapshots and SQLite/Lua state. Combat and environmental
vehicle casualty chains remain guarded pending integration; this adds the shared
injury and evacuation foundation rather than enabling those callers.


### Character injuries from vehicle criticals

Mech impact injuries and vehicle crew-hit/Gauss-explosion injuries now use one
health dispatcher. Assigned in-character pilots use character health and recovery;
virtual crews use tactical health. Critical reports retain direct character injury
results separately from tactical injuries, with nested explosions retaining their
own ordered results. The vehicle critical action publishes notices, visible
broadcasts, character feedback and newly fatal evacuation in one rollback checkpoint.

Tests compare crew-critical health and random streams with the shared standalone
injury action, cover fatal evacuation rollback, and verify surviving Gauss-explosion
injuries after internal damage and SQLite reload. Instant crew-kill criticals,
fatal hull damage and vehicle catastrophes still require their distinct character
casualty rules; direct character vehicle firing remains guarded.


### Instant vehicle crew death

Crew-killed criticals now record a distinct persistent crew-loss cause, exposed as
`crew_killed` in Lua state. They stop vehicle controls and release the pilot without
changing tactical injury counts, character health, armor or ammunition. All three
critical tables use the shared evacuation action, including vehicles without an
assigned pilot. Nested weapon explosions finish their material damage after crew
loss and publish evacuation through the same action checkpoint.

Tests cover each table, assigned and virtual crews, wizard exemptions, invalid
active-wreck snapshots, failed evacuation rollback, SQLite/Lua state and nested
Gauss explosions. A hotloaded explosion that also destroys the hull still rolls
back: fatal hull and catastrophe casualty rules remain unfinished, as do direct
character vehicle firing and VTOL integration.


### Vehicle hull loss and catastrophe occupants

Ordinary hull destruction leaves occupants alive inside the wreck. FASA power-plant
CASE containment destroys the rear section without killing the crew; fuel and
uncontained power-plant explosions mark crew death and destroy all remaining
sections. Nested critical explosions and remaining internal damage finish within
the same candidate. Character health is not rewritten to represent these deaths.

Located armor-damage and critical actions share notification, character feedback,
casualty publication and rollback. Tests cover ordinary hull survivors,
through-armor crew criticals, containment, fuel explosions, nested weapon/ammunition
cascades, failed evacuation, wizard exemptions and SQLite replay. Direct character
vehicle firing, environmental caller integration, transported-unit destruction and
VTOL support still need work. Excluded unit classes retain their admission guards.

Ordinary hull loss from terrain or scheduled section fires likewise preserves
occupants. Fire damage still rejects changes to character injury or crew-death
state because its publication path is unfinished; a seeded crew-kill fire test
verifies complete rollback, while hull-only fire tests verify surviving occupants.


### Vehicle fire character publication

Scheduled fire ticks retain both occupant notices and ordered character injury
reports. Their server action publishes injuries and newly lethal evacuation in the
same rollback checkpoint as armor damage. Terrain-fire entry retains these reports
through the existing movement action. Shared vehicle injury traversal preserves
nested critical order without repeating health or evacuation logic. Raw movement
continues to reject character crew consequences that require the host action.

Tests cover nonfatal/fatal scheduled injury, crew-kill exposure, terrain-fire
movement evacuation, failed afterlife rollback and persisted replay. Existing fire
save-failure tests exercise the server transaction. Inferno/heat attack admission,
other character combat callers, transported-unit destruction and VTOLs remain
unfinished; autopilot, repairs and excluded unit classes remain out of scope.


### Character vehicle inferno and blast heat

Inferno and heat exposure actions now share vehicle damage publication, ordered
character injury feedback and fatal evacuation. Standard mobile heat preserves
its explosion check, including zero heat; advanced fire retains section damage
and later fire timers. Mine and artillery actions publish both material-packet
injuries and subsequent heat injuries through one shared blast visitor. Raw blast
APIs still require non-character victims because they do not publish casualties.

Tests cover the explosion threshold, zero-heat explosions, advanced-fire feedback
exactly once, fatal evacuation rollback, mine-field rollback, artillery cursor
rollback and persisted replay. Direct character weapon firing, remaining movement
casualties, transported-unit destruction and VTOL integration are still unfinished.


### Shared electromagnetic and seismic sensing

Physical sensor queries, scanner acquisition and electromagnetic terrain visibility
now support mixed Mech, ground-vehicle and VTOL observers and targets through the
common scanner projection. Vehicle launch emissions and independently saved sensor
signal streams follow the same heartbeat lifecycle as Mechs. Failed Streak locks
do not mark emissions.

Regression coverage exercises every source/target family pairing, signal and
emission replay, independence from attack dice, sensor switching, airborne VTOL
exclusions, hover/stationary seismic target exclusions and fixed-installation
range boundaries. Unit loading, unloading and container cargo remain deferred.


### Infrared across supported chassis

Infrared unit queries, acquisition and aiming accept Mechs, ground vehicles and
VTOLs through common visibility and thermal-contrast rules. Ground vehicles and
VTOLs do not run the normal thermal-production cycle; their weapon-heat storage
does not become infrared heat production. Mechs retain their existing production
and cooling calculation.

Fixed installations have a 21-hex infrared hardware range; mobile units retain
15 hexes. Unit and terrain queries share this limit and the battlefield ceiling.
Tests cover mixed chassis, secondary acquisition dice, underwater vehicles,
read-only aiming, stored vehicle weapon heat, database replay and range boundaries.


### Character vehicle terrain firing

Character-controlled ground vehicles and VTOLs can fire at coordinates through
the shared host firing action. The existing launch, terrain impact, injury and
evacuation handlers own these consequences; raw tactical terrain shots retain
their non-character restriction. No additional launcher or casualty path is used.

Tests cover every coordinate mode with ground vehicles and airborne VTOLs,
native/Lua equality, callback rollback and database replay. Seeded rapid-fire
misloads verify character injury, crew death and complete rollback when the
evacuation destination is unavailable. Indirect terrain firing is covered below.


### Occupied spotter coordinates

A conventional spotter may select a coordinate containing a unit. Shared target
resolution chooses the first live occupant in battlefield order, excluding the
firer, and rechecks the spotter's current contact. Mech and vehicle firers use the
same indirect shot path even when they cannot see the target themselves.

Coordinate spotting retains coordination, movement and lock costs but omits the
spotter's terrain aim modifier, sensor aim dice and spotting skill awards. Tests
cover all Mech/vehicle shooter-observer pairings, native/Lua firing, replay,
callback rollback, unchanged query state and a target leaving the coordinate.
Empty spotter hexes use the terrain-shot integration described below.


### Empty-coordinate indirect fire

Conventional indirect weapons can fire at an empty spotter coordinate through
the common terrain-shot transaction. The observer supplies visibility and shared
coordination, movement and lock costs; range and weapon expenditure remain owned
by the firer. An occupied coordinate continues through unit-target combat.

A firer's unit lock overrides the observer. Without a unit lock, its terrain mode
controls clearing/ignition behavior, while a firer with no coordinate lock can
still use the observer's coordinate. Indirect launches bypass the firer's weapon
arc and direct visibility checks, but retain weapon, crew and observer admission.

Tests cover all Mech/vehicle shooter-observer pairings behind blocked sightlines,
terrain impacts, native/Lua parity, no firer lock, observer loss and visibility
loss, rollback of inventory/terrain/dice, and database replay.


### Heading during Mech jumps

Bipeds and quads accept heading commands while jumping. The shared movement
heartbeat changes facing independently of the committed jump path, position and
landing destination. Ground and jump rotation share shortest-arc normalization
and overshoot handling.

Normal jump turning uses three degrees per current gravity-adjusted whole jump
MP each second, doubled for quads. FASA jump turning is eighteen degrees per
second. Surviving thrust is sampled each update. Uncontrolled descent retains its
heading-control restriction.

Tests cover both chassis and policies at multiple gravities, jet loss, wraparound,
no overshoot, native/Lua controls, callback rollback, database replay and identical
flight trajectories with and without facing changes.


### Ground vehicles on authored terrain

Building, wall and high-water map tiles now reach the shared vehicle elevation
and hazard rules. The blanket unsupported-terrain stop has been removed. Tracks,
wheels and hovercraft can traverse level and one-level transitions; steep faces
still use the existing avoidance and crash consequences. High-water falls retain
their water damage reduction.

Regression scenarios compare building/wall movement with equivalent grass height
profiles and cover high-water traversal, steep barriers, smoke, preserved material
on ordinary travel and database replay for each drivetrain.


### Shared indirect-fire experience

Mech and vehicle shooters now use the same pre-impact spotting and artillery
skill awards, including on misses. The common helper applies a newly earned
spotting level to the current shot. Vehicle reports retain accepted awards in
`experience_messages`, and the host publishes them in the firing transaction.

Tests rotate both shooter and observer chassis through character eligibility,
disconnection, cooldown and coordinate-lock exclusions. They also verify current-
shot level gains, callback rollback of balances and messages, and saved awards
and levels after restart.


### Shared operator unit inspection

`@btech inspect` renders power, fall state, mass, AutoFall, destruction, motion,
pilot and map position through one formatter for Mechs and vehicles, including
VTOLs. Registrations no longer carry an unconditional inactive label. Status and
operator help describe the supported chassis and retain the incomplete-parity
qualification and deferred transport scope.

The completion audit still has confirmed open behavior: `mine_blast.rs` rejects
negative strengths, while the reference area-blast traversal accepts signed
damage and separately applies heat and neighbor effects. Supporting this requires
characterizing negative inferno burn durations and replacing tests that currently
use negative strength as an injected transaction failure. The existing positive-
strength tests do not prove this behavior.

Reference characterization for that open item: `combat/artillery.c` skips material
packets when damage is nonpositive but still sends cell notices and applies heat.
Command/vibra neighbors are visited when signed damage divided by two is nonzero,
so strengths at most -2 can still ignite neighboring woods. `combat/mech_fire.c`
adds signed heat times six to a Mech's existing burn timer;
`mux/network/mux_event.c` clamps scheduled delays below one to one second. Thus a
negative inferno strength shortens an existing burn, or creates a one-second burn
if none exists. Vehicle heat exposure follows its own fire-check policy. These
effects must share the existing blast and thermal owners across chassis; casting
negative strength to unsigned damage or treating the whole blast as a no-op would
change behavior.


### Signed mine strengths

All conventional mine types accept negative strength. Nonpositive material damage
applies no packets or location dice. Command/vibra strengths at most -2 still
visit neighboring hexes, emit blast notices and ignite eligible woods. Weak
fields are removed through the ordinary mine transaction.

Inferno heat remains signed: Mechs and stationary vehicles share one burn-timer
adjustment, with a minimum one-second timer for nonzero exposure. Negative heat
shortens an existing burn; mobile vehicles retain their configured fire/explosion
checks independent of heat magnitude. Blast `burn_seconds` reports the signed
adjustment; the stored timer remains nonnegative. Vehicle blast-heat APIs accept
signed heat. Missile inferno APIs remain positive-hit operations.

Tests cover bipeds, quads, tracked/wheeled/hover/stationary vehicles and landed
VTOLs; minimum signed strength, -2/-1/zero boundaries, existing burns, both fire
policies, neighbor ignition, deterministic replay and restart. Radio command-mine
tests include negative strengths through native/Lua dispatch and callback abort.
Late-failure regressions now inject unavailable woodland-ignition randomness
after earlier effects, preserving rollback coverage without rejecting valid mines.
This closes the negative-strength gap identified by the operator-inspection audit.


### Shared mine and artillery target selection

Mine and artillery cells now use `blast_damage::BlastCell` for live occupancy,
exclusive height bounds, facing and pilot Toughness. Selection remains inside
the ordered target loop, after preceding consequences. Each caller retains its
height origin (water bed for mines, water surface for artillery), blast origin,
character-publication guard, messages and terrain effects. Material packets and
signed heat already share the same resolver. Existing height-boundary, character,
vehicle, signed-mine and replay scenarios exercise the extracted path.


### Facing during forced descent and quad turning

Running Mechs and vehicles can request heading changes during forced descent,
subject to existing pilot, movement, fortification and hull-down restrictions.
One facing update uses the shared turn calculation and loaded movement ceiling;
descent cursors and translation retain their existing owners. Ground vehicle
descent validation permits a desired heading while still forbidding translation.

The reference heading command in `movement/mech_move_controls.c` and event
scheduling in `unit/mech_lifecycle.c` do not exclude falls. Its
`movement/mech_update_motion.c` applies the quad multiplier to ground turning as
well as non-FASA jumps. Rust now supplies that multiplier to the common turn
calculation; fixed FASA jump turning remains unchanged.

Regression coverage compares turning and non-turning falls for bipeds, quads,
tracked/wheeled/hover vehicles and VTOLs in both turn modes. It verifies rate,
wraparound, no overshoot, unchanged descent progression, native/Lua parity,
callback rollback and saved replay. Ground turns also verify the quad rate.

Falling VTOL heading events also call the ordinary fuel routine before turning.
Its low-speed draw, configured free-fusion policy and exhaustion handling remain
shared with powered flight. Tests include exhausted and already-announced empty
tanks and verify deterministic fuel draws alongside the independent descent.

The facing phase runs after ordinary motion and descent: aircraft that recover
lift have already used powered turning and are excluded from falling-unit facing.
A same-tick recovery regression verifies that the heading advances only once.


### Case-insensitive equipment identities

Weapon and system identities, ammunition namespaces, Artemis slot references,
split markers and TSM slot detection accept ASCII capitalization variants. The
shared equipment resolvers retain typed identities and preserve authored text
for inspection and persistence. Unknown names, malformed mounts and unsupported
modes still fail construction.

The reference `unit/mech_partnames.c` lowercases both registry keys and lookup
requests. Rust tests cover every weapon identity, mixed-case critical runs on
Mechs and vehicles, split mounts, unchanged loadout/mass and saved raw metadata.

A fresh audit of the 1,745 game assets finds 1,249 constructible assets among
1,302 biped/quad Mechs and 269 among 269 in-scope ground/VTOL vehicle assets.
Four Mechs now construct without asset edits: BloodKite-1, Hellion-A, Hellion-Prime
and PXH-IIC. No previously constructible asset regressed. The remaining 53 Mech
rejections include malformed or incomplete equipment and unsupported features;
these require individual classification. Constructibility does not prove full
simulation parity.


### Targeting-computer weapon eligibility

The shared eligibility query now admits HeavyFlamer, VehicleFlamer and
VehicleHeavyFlamer, matching their ammunition-fed category in the reference
weapon catalogue. Ordinary IS/Clan Flamers and machine guns remain excluded.
Artillery is excluded explicitly; zero missile count alone does not identify a
weapon that can use a targeting computer. All eight supported artillery identities
are covered by the eligibility regression.

The same query owns construction-mode admission, Mech and vehicle aiming, and
called-shot eligibility. Regressions cover the three flamer variants through
Mech native/Lua aiming and damage, vehicle attacks on both target chassis,
computer critical loss, read-only queries and saved replay.

Authored OnTC links are handled separately from automatic hardware eligibility;
see “Authored targeting-computer links” below for the completed integration.


### Repeated template comments

The shared Mech/vehicle parser accepts repeated case-insensitive `Comment` fields
and retains their text in source order, separated by newlines in
`attributes.comment`. Comments do not change the active section or create
equipment. Size and delimiter limits still apply, and duplicate ordinary unit
fields still fail. The reference loader's Comment case carries no game state.

Tests cover Mechs, ground vehicles and VTOLs, comments within sections, unchanged
loadouts, malformed input, size bounds, and Grendel-Prime inspection through
native and Lua commands. A fresh 1,745-file audit now constructs Grendel-Prime
without editing the asset and finds no construction regressions: 1,250 of 1,302
biped/quad Mech assets construct. The remaining 52 require further classification;
vehicle construction coverage remains 269 of 269 from the preceding audit.

### Case-insensitive ammunition rows and initial sizing

The shared source parser now recognizes the `Ammo_` prefix without regard to
ASCII case, including rows with whitespace-separated flags, numeric brands and
trailing dash placeholders. Mech construction uses the same prefix query when
normalizing initial bin quantities. Equipment spelling remains authored metadata;
live ammunition is still restored without construction-time refilling.

Regressions exercise source parsing and construction for Mechs, ground vehicles
and VTOLs, shared loadouts and mass, saved identity, and lowercase names across
all ammunition-using weapons and the existing quantity/half-ton boundary cases.

The rewrite's Daishi-H template uses whitespace-separated `Clan FlipArms`
Specials flags, as required by the Specials grammar. Daishi-H now constructs
with six weapons and four ammunition bins. The full asset audit finds no
construction regressions; 1,251 of 1,302 biped/quad Mech assets now construct.
The reference asset is unchanged.

### Authored template syntax corrections

Eight rewrite assets now construct after syntax-only corrections:

- ARC-6W and SGT-8R use pipe separators between weapon mode flags.
- Dra-A and PhoenixHawk-IIC2 include missing mode placeholders.
- Koshi-P, Ryoken-H and SD1-OD retain inactive equipment notes as flat Comment
  text, without nested opening braces. Koshi-P remains unarmed; its commented
  weapons have not been activated.
- Uller-D's left-arm ammunition occupies slots 8–9, preceding the equipment in
  slot 10, replacing the invalid descending range 8–0.

The complete asset audit finds all eight newly constructible and no regressions.
The current Mech count is 1,259 of 1,302 biped/quad templates; 43 remain rejected.
No reference-tree assets or Rust construction rules changed in this correction.

### Conflicting critical-slot corrections

Three rewrite templates now have non-overlapping equipment allocations:

- BlackHawk-H retains five two-slot heavy medium lasers per arm, occupying pairs
  3–4 through 11–12 after the shoulder and upper actuator.
- Supernova-1 retains three ER large lasers per arm in slots 5–7. Its four arm
  heat-sink slots move to unused positions 8–11, preserving installed equipment.
- BloodAsp-Prime removes the right-hand actuator row that conflicted with the
  heavy medium laser in slots 4–5, matching the handless left-arm layout.

The asset audit constructs these units with 10, 6 and 7 weapons respectively and
finds no regressions. Current construction coverage is 1,262 of 1,302 biped/quad
Mech templates, with 40 still rejected. Rust overlap and installation validation
remain unchanged, as does the reference tree.

### Mixed relocated engine installations

Complete Inner Sphere torso installations may retain seven center criticals and
mixed side counts after relocation. Ten-slot layouts with a one-slot side and
twelve-slot layouts with a one- or three-slot side now resolve as XL, including
mixed Light/XL side evidence. The reference's per-section technology discovery
sets XL for one or three slots; XL takes precedence in engine identity and the
implemented defensive BV calculation. Physical critical locations are retained.

AXM-3S, HBK-5S, MAD-4S, STK-8S and BTZ-3F now construct without asset changes.
Regression coverage checks mirrored layouts, mass, defensive battle value,
three-hit destruction, retained installation identity and saved damaged units.
Incomplete totals, inconsistent side evidence and non-torso engines remain
rejected. A complete asset audit finds these five newly constructible and no
regressions: 1,267 of 1,302 biped/quad Mech templates construct, leaving 35.

### Consistent launcher manufacturer metadata

Eight rewrite assets now use consistent manufacturer metadata across each
multi-slot weapon. BlackLanner-B, Lan-B and Nig-D retain LRM brand 5;
EXT-5E and KTO-21 retain iNarc brand 3; RFL-IIC_SJ retains UltraAC/2 brand 5.
Tur-P and Turkina-Prime use explicit zero throughout their unbranded LRM mounts,
replacing the mixture of zero and omitted brand fields. Weapon identities,
counts, modes and critical locations are unchanged.

The complete construction audit finds all eight newly constructible with no
regressions. Coverage is now 1,275 of 1,302 biped/quad Mech assets; 27 remain
rejected. Rust mount consistency checks and the reference tree are unchanged.

### Arrow launcher metadata and ammunition flags

ANV-8M now uses manufacturer 3 across its entire Clan Arrow IV launcher.
Matador-1's machine-gun ammunition uses canonical `Hotload`; VKG-2F uses
canonical `Halfton`. Equipment identities, critical allocations and ammunition
quantities are unchanged. The full asset audit finds all three newly
constructible and no regressions: 1,278 of 1,302 biped/quad Mech assets construct,
leaving 24. The reference tree remains unchanged.

### Split Arrow IV mounts

The shared Mech mount resolver admits IS Arrow IV installations with explicit
split links. The extension count determines the primary run length, so a linked
mount need not fill the primary section to slot 12. Complete contiguous runs,
matching metadata, adjacency, valid parent pointers and exact total slot counts
remain required. Split eligibility is defined once on the weapon identity and
used by both link validation and mount construction.

CPLT-C5 retains its nine right-arm and six right-torso Arrow IV slots. Its torso
entries now explicitly link to the arm mount; the loadout contains one fifteen-slot
launcher. Regression coverage verifies damage in every primary and extension
slot, incomplete-link rejection and saved damaged-state round trips, alongside
existing split gun, section-loss and explosion tests.

The complete asset audit finds CPLT-C5 newly constructible with no regressions:
1,279 of 1,302 biped/quad Mech assets now construct, leaving 23 rejected.

### GOL-3S torso engine and split cannon

GOL-3S now keeps both left-side light-engine criticals in its torso. The LBX-20
occupies torso slots 3–12 plus an explicit extension in rear-left-leg slot 5,
replacing the engine slot previously placed there. The eleven-slot cannon and
all other equipment remain installed. This uses existing split-mount handling;
no Goliath-specific runtime rule was introduced.

The engine regression checks the corrected Light family and eleven-slot mount,
and still rejects an engine inserted into a leg. Engine and split-weapon tests
pass. The full asset audit finds GOL-3S newly constructible and no regressions:
1,280 of 1,302 biped/quad Mech assets construct, with 22 remaining rejections.
The reference tree is unchanged.

### Complete heat-sink groups in authored assets

Nine rewrite templates now allocate complete heat-sink groups without changing
their declared cooling capacity or installed weapon identities:

- HMR-3P completes its right-arm three-slot sink in slots 6–8.
- Nig-B and NightGyr-B complete their left-arm sink in slots 9–10.
- RFL-3NC completes its left-torso sink in slots 1–2.
- Clint-IIC completes its right-torso sink in slots 3–4 and moves the jump jet
  from slot 4 to previously unused slot 12.
- Naga-A, Naga-B, Naga-C and Naga-Prime move each incomplete torso sink to a
  complete pair in previously unused slots 6–7.

The complete asset audit finds all nine newly constructible and no regressions:
1,289 of 1,302 biped/quad Mech assets construct, leaving 13 rejections. Runtime
sink grouping remains strict, and the reference tree remains unchanged.

### Remaining conventional system corrections

Five rewrite assets now construct after completing their equipment definitions:

- AS7-DC uses supported Clan ER medium lasers for its four nonexistent
  `CL.MediumLaser` entries, retaining the two rear-facing mounts.
- WVR-6MCL installs its missing second life-support critical in free head slot 4.
- Piranha moves its incomplete head sink to a complete right-torso pair, 11–12.
- Piranha-1 completes both leg sink pairs in 5–6 and moves its incomplete center
  sink to left-arm slots 6–7.
- Masakari-C uses two left-arm sink pairs in 3–6 and completes its remaining
  partial sink in free right-arm slots 11–12.

Declared cooling fields remain unchanged. The complete asset audit finds these
five newly constructible and no regressions: 1,294 of 1,302 biped/quad Mech assets
construct, leaving eight rejections. No runtime construction checks were relaxed,
and no reference-tree files were modified.

### Authored targeting-computer links

Weapon mounts retain explicit `OnTC` independently of automatic catalogue
eligibility. One shared aiming query serves Mechs and vehicles: an explicit
link enables assistance, or an eligible weapon gains assistance from installed
computer slots; any unavailable computer slot disables it, and cluster ammunition
never benefits. Explicit links can exist without physical computer slots, matching
retained authored links in the reference loader. Computer installation/status
queries still require actual equipment. Automatic machine-gun eligibility remains
false; authored links do not alter the catalogue.

Ammunition `OnTC` is retained in template metadata but does not link a weapon or
change its ammunition mode. Goshawk-1, Goshawk-2, Thor-D and Viper-2 now construct
with their authored weapon links. Mas-A also constructs after its separate LRM
manufacturer mismatch is corrected to brand 5 across the mount.

Regression coverage checks automatic versus explicit links, absent and damaged
hardware, cluster exclusion, vehicle attacks on Mechs and vehicles, damage loss,
read-only aiming and saved replay, plus preserved authored Mech links and inert
ammunition flags. The full asset audit finds five newly constructible and no
regressions: 1,299 of 1,302 biped/quad templates construct. The three remaining
rejections contain LAM conversion equipment, whose flight behavior remains
outside the current aerospace scope. Other gameplay parity still requires audit.

### Persistent TIC membership

Mechs and vehicles share four ordered weapon groups numbered 0–3. `addtic`,
`deltic`, `cleartic` and `listtic` use the same membership operations as
`btech.unit.tic` and `btech.unit.tic_edit`. Native selections support numbers,
inclusive ranges and comma-separated lists; `deltic <group>` clears that group.
All changes require the conscious assigned pilot physically in the cockpit.

Selection validation precedes mutation. Weapon damage does not renumber or remove
members; group state persists with the owned unit and is validated against the
installed weapon count on loading. Tests cover Mechs, ground vehicles and VTOLs,
duplicate additions, invalid edits, wrong-pilot access, callback rollback,
critical loss, saved replay and corrupt group members. Group firing remains
unfinished and is the next acceptance item in the delivery audit.


### Ordered TIC firing

`firetic <groups> [#unit]` and `btech.unit.tic_fire(unit, pilot, groups, target?)`
use the ordinary configured firing action for every selected weapon, including
launcher failures, expenditure, damage, crew consequences and observer messages.
Groups and weapon numbers run in ascending order; overlapping groups retain
repeated attempts and ordinary recycling checks. A rejected shot restores that
attempt and continues. Recoil falls and shutdown stop the batch. Publication
failures and failed Lua callbacks restore the entire batch and its messages.
Empty groups click only after startup, map and cockpit authority checks.
`listtic` now uses the common weapon readiness display, including damaged mounts.

Cross-chassis tests cover bipeds, quads, tracked vehicles and VTOLs, compare grouped
results with ordinary firing, and exercise duplicate selection, failed shots,
callback rollback, native/Lua agreement and saved replay. The Heavy Gauss recoil
scenario verifies firing stops before the next group. Target arguments currently
follow the ordinary Rust `fire` interface (explicit object or saved lock); remaining
reference target grammar belongs to the cockpit interface audit.

Validation: the complete suite passes 1,865 tests; all-target Clippy with warnings
denied, formatting and diff checks pass. The reference tree remains unchanged.


### Heat cutoff regulation

Mechs persist one heat-cutoff state containing its setting, intentionally disabled
cooling points and a four-second toggle countdown. `heatcutoff` and
`btech.unit.heatcutoff(unit, pilot)` share conscious assigned-pilot admission and
the `battletech.heatcutoff` gate. Admitted countdowns continue with the reactor off
and survive restart; failed Lua callbacks restore the pending toggle and notices.
`btech.unit.state(unit).heat_cutoff` and cockpit status expose the state.

The existing thermal sample applies water, inferno and temperature effects before
regulating stored weapon heat plus continuous production minus dissipation.
The regulator changes at most two points per sample (four for double/Clan sinks),
using the 9–10 band and restoring cooling gradually after disengagement. Current
sample water bonuses remain until recomputed on the next sample. Physical damage
remains separate; disabled capacity clamps to surviving cooling at the next
sample. Idle heartbeat admission includes pending transitions and restoration.
Vehicles retain their existing thermal model; this control belongs to Mechs.

Validation: the complete suite passes 1,869 tests. After matching the reference
command’s ignored trailing text, all three heat-cutoff integration tests pass
again. Final all-target Clippy with warnings denied, formatting and diff checks
pass. The reference tree remains unchanged.


### Preferred ammunition sections

`usebin <weapon> <section|->` and `btech.unit.usebin(unit, pilot, weapon, section?)`
share conscious assigned-pilot, map, weapon and section admission. Reactor power
and present matching ammunition are not prerequisites. Biped/quad and vehicle
anatomies supply typed persisted preferences to one feed-priority rule: preferred
section, mount section, then canonical section/slot order. Ordinary launcher,
burst fallback, misload and unjam consumers continue using the shared feed planner.
A preference never bypasses mode, supply or critical-availability checks.
Automatic missile defense intentionally ignores the preference, matching its
separate reference lookup request. Energy weapons, including catalogue Laser AMS
entries that retain ammunition fields, cannot select a preferred section.

Weapon inspection exposes `preferred_ammunition_section` and the common cockpit
weapon display shows it. Tests cover native/Lua agreement, failed callback rollback,
restart, actual expenditure, recycling admission, damaged/empty preferred bins,
and normal versus special ammunition fallback across supported chassis.

Validation: the full suite passes 1,872 tests. After matching ignored trailing
arguments, all five ammunition-feed/preference tests pass again. Final all-target
Clippy with warnings denied, formatting and diff checks pass. The reference tree
remains unchanged. The delivery audit records a separately discovered idle heat
cutoff discrepancy that still needs correction.


### Independent cutoff timing and automatic turret tracking

Idle stopped Mechs now finish heat-cutoff toggles without regulating cooling or
advancing thermal-check clocks. Both suppression and gradual restoration wait
until thermal accounting resumes. The lifecycle test covers both toggle
orientations and subsequent startup; the idle-server test verifies the saved
transition finishes without a map or reactor power.

`autoturret` and `btech.unit.autoturret(unit, pilot)` persist one mode on vehicle
state. Shared tracking reads either target anatomy through the scanner projection
or the selected hex, updates hull-relative facing once per committed second, and
does not wait for lock settling. Power, consciousness, surviving turret,
jam and lock conditions govern updates. Invalid, deleted and cross-map unit
references do not rotate the turret. Tracking precedes scanner refresh and shares
the server's rollback boundary. The integer bearing preserves signed tenth-degree
quantization and the 180-degree same-point convention.

Tests cover controls and callback rollback, live target movement, hex selection,
hull-relative facing, shutdown/jam/lock gates, saved replay and a running server.
Ground propulsion types and turret-equipped rotorcraft use the same implementation;
rotorcraft without turrets reject the control. Sensor-flash blindness remains an
explicit acceptance gap, recorded in `btech-delivery.md`.

Validation: the full suite passes 1,876 tests. All thirteen cutoff/turret tests
pass after the final turning refinements: crew stun permits rotation, unconsciousness
pauses it, and fractional hull turns preserve the tracked world bearing. Final
all-target Clippy with warnings denied, formatting and diff checks pass. The
reference tree remains unchanged. Reactor explosions and their temporary
sensor-flash effects remain acceptance work in the delivery audit.

### Reactor blasts and temporary sensor blindness

A trusted callback can call `btech.unit.reactor_explode(unit)` to detonate a
constructed Mech. The action shares blast packets, heat exposure, neighboring
forest ignition and casualty publication with existing combat. It checkpoints
all world and notification changes, including failures in departure callbacks.

Visible observers with infrared or light amplification as their primary sensor
are blinded for four committed seconds. Saved recovery runs while stopped and
suppresses cockpit controls/readouts, visual broadcasts, spotting and automatic
turret tracking. Trusted unit state exposes `blinded_remaining`. Tests cover
cross-chassis blasts, sensor susceptibility, forest ignition, replay, casualties,
rollback, persistence and live-server recovery.

Engine-instability triggers, cockpit countdowns and terminal crew injuries are
implemented as described below. Ejected MechWarrior infantry remains within the deferred infantry
scope; the implemented fatal-crew path uses existing evacuation behavior.

Reactor milestone validation: all 1,880 tests pass, along with formatting,
Clippy across all targets with warnings denied, and diff checks. The reference
tree remains unchanged. The broader acceptance work is tracked in
`btech-delivery.md`.


### Cockpit self-destruction and terminal crew injuries

Native `explode` and trusted `btech.unit.explode(unit, pilot, text)` use one
controller and durable scheduler for Mechs and vehicles. `unit.state` exposes
`self_destruct` and `self_destruct_safe`; scenario callbacks can set the latter
through `btech.unit.explode_safe(unit, safe)`. The safety flag gates new ammo
requests. Configuration, cooldown, wizard override and in-character pilot
ownership checks follow the cockpit behavior; engagement releases the pilot.
Optional `MechDebugInfo` announcements share transactional publication.

Countdown state retains scheduling order and the reference's actual modulo-256
mode interpretation, including zero and negative configured delays. Mechs reuse
the existing reactor or ammunition explosion actions; vehicles destroy the rear
section and receive the final crew injury. Wreck placement first selects terrain
support, then adds six levels. Descent is scheduled according to the original
altitude, including the distinction between bridge decks and beds.

Terminal injury reuses tactical consciousness and recovery after pilot release,
preserving previous unit injury counts without adding personal-health damage to
an evacuated character. New wreck recovery survives restart. Self-destruct tests
cover seven chassis, native/Lua controls, admission exceptions, countdown order,
live ammunition cancellation, restart, callback rollback, failed server commits,
and elevated wreck placement across all supported vehicle classes.


### Reactor instability from section loss

Section destruction and flooding apply the configured stackpole check when
engine losses reach three. Ordinary engine criticals do not perform this check. It reuses the reactor blast resolver and retains nested reports
for atomic casualty publication, including ammunition and other damage cascades.
Eligibility includes 30 elapsed seconds after the first pristine center-torso
internal hit; that hit opens the window after its criticals. Initial world
startup retains the reference's zero-timestamp grace. A successful 2d6 roll of
nine or more requires a running or starting reactor; an eligible stopped reactor
still consumes the roll. Supercharger overload follows its separate failure rule.

Both bounded clocks expire on committed server ticks, persist across restart,
and roll back together with damage and notifications. Trusted Lua unit state
exposes `reactor_instability_remaining`; nil uses the initial world grace and
zero means the damage window expired. The `stackpole` and `explode_reactor`
settings supply runtime admission and hit-table policy.


### Manual reactor reentry and neighboring chains

Destroying a reactor manually uses the same engine-compartment instability check
as combat damage. When eligible, the nested explosion finishes before the
original blast. The report's optional `section_explosion` contains that event;
its notices and casualty effects share the caller's checkpoint. Recorded engine
losses bound reentry, while a fresh request after head destruction is rejected.
Tests cover both blast passes, terminal crew injuries, timing, power and replay.

A reactor can also trigger a neighboring reactor through ordinary blast damage.
The second report remains inside its triggering impact, and its blast can reach
the original wreck without retriggering it. Cross-unit damage, casualty callback
rollback and save/reload tests cover the complete chain. The reference's later
cleanup of fully destroyed in-character objects remains acceptance work.


### Shared Swarm and Swarm-1 missile flights

Implemented typed supplies, native/Lua controls, weapon/status display and a shared
retargeting resolver for every supported launcher/target class. Initial misses end
the flight; secondary misses retain missiles. Map order, contact visibility, strict
1.9-hex selection, cumulative range, immunity/friendly filtering, self-retargeting,
AMS exemption and the eleven-attack visited-slot boundary are covered. Each hit
reuses existing capped weapon groups and anatomy-specific damage, with per-hop
publication inside the launcher's transaction.

Nine new tests cover the compatible ammunition catalogue, all 49 supported chassis
pairs with restart replay, secondary selection/damage, range and random-stream
boundaries, installed AMS, and native/Lua multi-target callback rollback. All 1,966
tests pass; all-target Clippy with warnings denied is clean. Fresh construction
audits retain 1,299 Mechs and all 269 supported vehicles/VTOLs. The three LAM
constructions remain excluded. The reference tree is unchanged. Full integration
is still incomplete; the current acceptance work remains in `btech-delivery.md`.


### Shared cluster and missile special-round controls

FIRECLUSTER/CLUSTER now share Mech and vehicle ammunition admission/storage.
FIRESMOKE/FIREMINE and Lua equivalents follow the reference's actual missile
eligibility rather than its misleading artillery help text. Typed missile
Smoke/Mine supplies use ordinary damage; Mine bypasses AMS. Authored artillery
payloads retain their environmental arrival effects. Cross-chassis control,
rollback, persistence, queued cluster arrivals and missile interception tests
cover the change.

A full run exposed an unseeded neighbor's reactor breach in an ice-landing test.
The blast ignited a unit that then produced steam in water. The landing-order
fixture now seeds that neighbor without changing production physics or weakening
its terrain assertions. All 105 surface tests and 40 repeated landing cases pass.
All 1,970 tests pass in the final full run; all-target Clippy with warnings denied,
formatting and diff checks are clean. The reference tree is unchanged. The active
acceptance audit continues with shared weapon-mode admission and SIGHT.

### Underwater conventional combat

Underwater sighting and conventional unit/coordinate fire now use the shared
mount-specific water ranges and weapon eligibility, superseding the earlier
blanket-rejection notes above. Water C3 retains the physical range cap; optional
range-based energy damage uses water bands before glancing. Native/Lua, rollback,
restart, both target anatomies and blocked waterline behavior are covered in
`tests/btech_underwater_fire.rs`. Unsupported families remain separate work.
See `docs/porting-audit-progress.md` for current verification results.

### TAG on supported vehicles

TAG now shares selection, ownership, timers and damage/range validation across
Mechs and tracked, wheeled, hover, stationary and VTOL units. Either family can
illuminate targets from the other, and vehicle TAG supplies the existing
semi-guided missile aiming rule. C3 master computers also provide integrated
TAG. Vehicle records, shutdown, membership resets, scheduling, Lua inspection
and status carry the shared TAG state. Native TAG accepts battlefield IDs and
uses distinct missing-equipment, destroyed-equipment, invalid-target and
out-of-range replies. Cross-chassis, rollback, restart and missile-assistance
coverage is in `tests/btech_tag.rs`; see the progress log for focused validation.
Earlier Mech-only TAG descriptions are superseded by this implementation.

### Character advantage kinds and boolean effects

The shared catalog classifies all 22 advantages as boolean, ranked or an
attribute mask. `btech.character.advantages()` returns detached canonical names
and kinds. Existing gameplay boolean checks now consistently require stored
value one and accept case-insensitive names; other positive values do not grant
boolean effects. Raw records and metadata remain inspectable without rewriting
saved inputs. Injury, movement, shutdown and physical-combat checks share this
interpretation. Remaining advantage effects and character generation are still
open; the audit records the reference's stale numeric life-accounting coupling.
