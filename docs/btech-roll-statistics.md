# Dice statistics report

The reference command is wizard-only `+rolls` (`CS_NO_ARGS`), registered in
`src/mux/commands/command_table.c`. The earlier audit's `@stat` name was incorrect.
`src/btech/ui/mech_stat.c::do_show_stat` reports the generic roll histogram from
`BtechContext::random.statistics`. It is not a unit-status or general database
statistics command. The Rust histogram and renderer are implemented; the command and live accounting integration remain open.

The report has eleven rows, sums 2 through 12. Each row shows the observed count,
observed percentage, expected count truncated to an integer, expected percentage,
and cumulative hit/miss probabilities for meeting that target. The theoretical
weights are 1,2,3,4,5,6,5,4,3,2,1 out of 36. Calculations use float precision before
formatting three decimal places. Empty statistics produce
`No rolls to show statistics for!`; the report ends with `Total rolls: N`.

The exact header is:

```text
#    Rolls %Current  Optimal Rolls %Optimal  %Hit Chance  %Miss Chance
```

`unit/mech_systems.c::btech_random_roll` draws two six-sided dice and calls
`btech_context_roll_record` exactly once. That record operation increments one
histogram bucket and the total. Separate hit-roll and critical-roll recording
functions update different arrays, which this command does not display.
`character/btechstats.c::char_rollskilled` and `char_rollsaving` use direct dice;
the fact that a result is between 2 and 12 does not make it a generic counted roll.

Integration must classify the actual reference call sites before wiring Rust's
shared dice APIs. Counting all random draws, all two-dice results, or only combat
attack rolls would give different statistics. The Rust engine also uses discarded
candidate worlds and independent persisted streams. A report must not accidentally
count repeated prediction calculations, drop historical counts when a unit is
removed, or disclose private generator state. The ownership and lifetime of the
histogram need an explicit design; a placeholder report with partial counts is
not completion of this requirement.

Read-only inspection found generic-roll call sites in combat damage, hit-location,
critical, missile, fire, physical and environmental resolution; boosters,
overheat and orbital descent; map objects/buildings; electronics, radio and unit
events; and character experience. Deferred battle-suit and repair calls are also
present in the reference. This is a source inventory, not acceptance of Rust
accounting for those paths.


## Histogram and renderer checkpoint

`BattleRollStatistics` owns eleven private counts and derives its total without
a second mutable counter. `record` accepts an explicitly classified sum from 2
to 12 and rejects invalid inputs or overflow without partial updates. Counts do
not expose generator state. The value can be cloned into a discarded candidate
without changing the original. Its report matches the reference empty message,
header, field widths, three-place percentages, cumulative hit/miss probabilities,
f32 conversion order and truncated expected counts. Public diagnostic counts are
u64 rather than inheriting the reference's signed-int overflow.

Two unit tests verify exact empty/edge/middle rows, all buckets, totals, read-only
rendering, independent candidates, invalid input and overflow. They pass in
`target/audit-roll-statistics-renderer.log`. Those early renderer tests cover the
foundation; the live command and lifecycle checks below cover integration.

Integration requires simulation-lifetime ownership, classification of
reference generic-roll callers against Rust rule entry points, and publication of
counts at the same transactional boundary as gameplay. `Scripts::call` reaches
`lua::transactions::run`, but native/server simulation paths also need coverage;
a Lua-only collector would be incomplete. The report must retain counts when
units are deleted, omit direct character dice, avoid double-counting discarded
world candidates, and preserve prediction/read-only behavior. Restart lifetime
must follow the reference's process-owned statistics, not silently accumulate
unrelated persisted per-unit totals.

All 241 library tests pass in `target/audit-roll-statistics-unit.log`; formatting
passes and reference files are unchanged.


## Explicit generic-roll journal

`BattleDice::generic_roll` now records one sum in a private per-stream journal.
Plain `two_d6`, `d6` and consciousness dice do not count automatically. Journals
clone with candidate worlds, making a discarded clone independent of its source.
Generator equality deliberately compares the random stream only; diagnostics
are inspected through `generic_roll_statistics`, and
`take_generic_roll_statistics` transfers a journal without drawing random words.
The journal is process-local and omitted from persistence. Serialization delegates
directly to the tagged generator, preserving its full-width position; serde
flattening was rejected by tests because its buffered deserialization does not
support that u128 field.

Initial classified callers:

| Rule | Reference path | Rust accounting |
| --- | --- | --- |
| Ordinary weapon attack | `combat/mech_fire_preparation.c::weapon_fire_roll` calls `btech_random_roll` | `BattleWeapon::attack_roll` uses `generic_roll` |
| Dead-fire attack | Same function draws three direct d6 | Remains uncounted |
| Extended LRM below minimum | Same direct three-die branch | Remains uncounted |
| Extended LRM at/above minimum | Generic branch | Counted once |
| Caseless propellant failure | Generic check in `combat/mech_fire_resolution.c` | `launch_roll` counts the additional check |
| Heat-override Computer check | Character skill dice | Remains plain/direct dice |
| Mech and vehicle piloting | `unit/mech_identity.c` pilot checks | Counted after admission; automatic support and blocked controls draw nothing |
| Orbital landing | `movement/mech_ood.c` | Counted for either supported anatomy |
| Booster failure | `movement/mech_boosters.c` | Generic check counted |
| Heat ammunition and ordinary shutdown | `movement/mech_overheat.c` | Generic checks counted; Computer override excluded |
| Woodland ignition and clearing | `combat/mech_terrain_effects.c` | One direct ignition check, or two clearing checks plus conditional ignition; duration/replacement dice excluded |
| Fire spread | `map/map_obj.c` | Generic check counted; separate spread/duration dice excluded |
| Analog radio distortion | `ui/mech_notify_radio.c` | Generic check counted after percentile admission; percentile excluded |
| iNarc pod swatting | `sensors/mech_electronics_controls.c` | Admitted swat counted on the candidate world |
| Exposed searchlight damage | `combat/mech_damage.c` | First check and conditional second check counted |
| Weapon-table locations and FASA/critical-proof rerolls | `combat/mech_hitloc_standard.c`, `mech_hitloc_fasa.c`, `mech_hitloc_critproof.c` | Generic location draws counted, including fall/blast/physical/charge/DFA/Narc callers; punch/kick d6 excluded |
| Vehicle motive and advanced critical selection | `combat/mech_hitloc_motive.c`, `crit_vehicles.c` | Generic checks counted; standard single-die critical choices excluded |
| Damage entry, rear tow check, TAC/AP and internal criticals | `combat/mech_damage.c`, `mech_armor_damage.c` | Generic draws counted even when diagnostic-only; both anatomy stores covered |
| Enhanced weapon criticals | `combat/mech_enhanced_criticals.c` | Generic degradation check counted |
| Physical, charge and DFA attacks | `combat/mech_physical_resolution.c`, `mech_charge.c`, `mech_physical_damage.c` | Admitted attack checks counted |
| Cluster and swarm continuation | `combat/mech_combat_missile.c` | Ordinary clusters and subsequent swarm attacks counted; hotloaded three-die clusters excluded |
| Clan AMS | `combat/mech_combat_missile.c::mech_ams_intercept` | Generic two-die check counted; IS single-die check excluded |
| Immobile aimed-location selection | `combat/mech_hit_resolution.c` | Generic check counted; targeting-computer single die excluded |
| Sixth-sense warning | `combat/mech_combat.c` | Admitted warning check counted; delay die excluded |
| Reactor explosion and vacuum breach | `combat/environment_damage.c` | Generic checks counted after their existing guards |
| Cocoon interception | `combat/mech_damage.c` | Generic check counted for either anatomy |
| Vehicle heat explosion and fire exposure | `combat/mech_combat_misc.c`, `mech_fire.c` | Generic checks counted; single-die heat damage excluded |
| Perception XP attempt | `character/character_experience.c` | Admitted generic check counted despite its character-skill target |
| RAC unjamming | `unit/mech_events.c` | Generic check counted |

Tests verify ordinary versus special attack classification, one extra caseless
check, identical random outcomes and subsequent stream state, journal isolation,
drain behavior and fresh journals after deserialization. Woodland tests additionally
cover exact histogram buckets and subsequent generator state across 2,304 combinations
of seed, terrain and intent. Piloting assertions distinguish blocked and automatic
checks from a real roll. A cluster matrix checks normal/hotloaded/energy behavior
and exact journal buckets without changing random outcomes. The production
`two_d6` callers are now classified; the remaining ordinary call is the deliberate
Computer skill override. This search alone does not prove all reference call
sites have corresponding runtime behavior: missing rules and ownership boundaries
still require acceptance against the reference inventory.
`World::battle_roll_statistics` now combines live unit/map journals with retained
history. The wizard-only `+rolls` command uses that read-only aggregate.

The transactional `World` root owns skipped `btech_retired_rolls`. Queries merge
the live journals read-only; immediately before removal, `retain_battle_rolls`
preflights merges and then drains selected journals into that history. Repeating
retention cannot double-count. `dbck::plan` and delayed wreck retirement call this
before their respective `BtechState::purge` and `wreck_cleanup::forget` operations.
Wreck departure callbacks run before retention; subsequent movement callbacks
and SQL publication retain the existing world/effect rollback boundary.
Maintenance callback replay starts from the pre-repair world. It therefore
retains the post-callback journals and purges BattleTech identities again when
publishing tombstones, rather than copying the planner's earlier history. This
also keeps callbacks' live changes to surviving units intact. Aborted Lua cleanup
restores both retained and live journals with the world checkpoint.
Purge departure retires a unit's simulation identity after leave callbacks and
before clearing its object location. Keeping placed identity through that point
violates the battlefield/containment invariant. Move callbacks therefore see the
retired identity, matching the existing delayed-wreck sequence; their failures
still restore the pre-departure world and history. The final maintenance purge
handles maps and any remaining identities without counting drained journals twice.

Map replacement carries the old fire stream and its journal forward. Ordinary
creation rejects an occupied BattleTech identity. Vehicle ammunition-cascade
replacement clones the original vehicle, preserving its journal. Persistence
loads construct fresh journals; persistence validation uses detached state copies
and does not mutate the live history.
The remaining direct `two_d6` occurrences
include unit tests and character exceptions; they must not be renamed blindly.

Verification: all 243 library tests pass in `target/audit-roll-journal-unit-complete.log`.
All 51 special-round, sprint and vehicle-fire integration tests pass in
`target/audit-roll-journal-combat.log`. Formatting passes and reference files are
unchanged. The earlier library runs recorded the serde-flattening failure before
direct generator serialization fixed it; they are not passing baselines.

## Live command and inventory boundary

`+rolls` is registered with the reference wizard permission, no switch table and
no argument interpretation. As with `CS_NO_ARGS`, trailing text is ignored.
Switches receive `Command +rolls does not take switches.` after permission
checking. The handler returns a literal report, without requesting a commit or
changing a counter. In-game help is `help +rolls`.

The reference generic-roll inventory includes repair skill checks in
`repair/mech_tech.c`, BattleSuit damage in `combat/bsuit.c`, and attached-swarmer
hit locations in `combat/mech_hitloc.c`. These remain with their deferred unit
and repair systems; they do not represent uncounted active Rust checks. The
building missile path in `map/map_buildings.c` is represented by the shared
cluster resolver in Rust coordinate shots before their building damage packets
are applied. Function definition `unit/mech_systems.c::btech_random_roll` is
not itself an additional caller. Other reference files are mapped in the table
above. Recovery and seismic streams only use direct consciousness or signal dice,
so neither contributes generic checks.

`tests/btech_roll_statistics.rs` verifies empty output outside a cockpit, wizard
admission, permission-before-switch precedence, ignored arguments, live piloting
counts for Mechs and vehicles, direct-dice exclusion, exact report text, repeated
read-only reports and fresh totals after persistence reload. Existing histogram,
cluster and lifecycle tests cover per-bucket counts, auxiliary dice, retirement
and aborted cleanup. This command reports the generic checks executed by the
implemented simulation; it does not claim implementation of deferred rules.
