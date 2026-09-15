# Unit combat statistics

The reference's active shot counters belong to the combat unit, independently
of its current pilot. The similarly named character values are catalogue
entries; reference combat does not increment them. The rewrite keeps that
separation.

`shots_fired`, `shots_hit` and `shots_missed` are available through the existing
unit field inspection and administrative setter, including native and Lua
adapters. They start at zero and retain signed 32-bit values across restart.
Administrators can edit each independently, so their sum is not a validation
invariant. An increment that would overflow rejects the entire shot transaction.

One launched direct-unit attack increments `shots_fired` and exactly one result
counter. The result uses the reference broadcast threshold, including the
configured near-miss band, before missile grouping and interception. Multiple
rounds in a burst still count as one attack. An ordinary out-of-range launch
counts as a miss. Coordinate-directed shots, including occupied-hex and
observer-directed fire, do not count. Failed Streak locks and loader failures
that prevent launch do not count.

Mechs and vehicles store the same small counter type and call one update rule.
The vehicle host passes coordinate intent into firing before effects resolve,
so that intent is available to both combat effects and statistics. The native,
Lua and low-level direct-shot paths share counter updates; the enclosing shot
or host checkpoint rolls them back with ammunition, damage and dice.

`tests/btech_shot_counters.rs` covers all seven constructed chassis, hits and
misses, native/Lua agreement, signed field edits, overflow rollback, callback
abort, restart, occupied-coordinate fire, failed Streak locks and physical
out-of-range launches. Existing firing tests retain their independent ammo,
heat, dice and damage expectations and explicitly include the new counters.

`damage_taken` and `damage_inflicted` now use shared signed storage and field
controls as well. Damage owners record the incoming material packet after
combat-safe and cocoon checks. VTOL rotor scaling precedes accounting; hardened
armor and reinforced/composite internal material rules follow it. Armor-to-
internal overflow and subsequent section transfers do not count the packet
again. An internal explosion is a new entry. Hits redirected from an already
destroyed Mech limb follow the reference continuation rule and do not increment
the initial-packet totals. Self/environmental damage increases only damage taken;
an explicitly attributed other unit receives damage inflicted. Both totals are
prepared before either is mutated, and overflow aborts the damage transaction.

`tests/btech_damage_counters.rs` verifies mixed chassis attribution, restart,
callback/overflow rollback, initial-packet overkill, combat-safe suppression,
rotor/hardened/reinforced ordering, standalone internal damage and destroyed-limb
redirection. Existing impact and salvo tests retain their independent material,
dice and lifecycle expectations.

Additional attribution checks exercise a normal kick, both glancing policies,
a miss and subsequent balance falls. Only the kick packet credits its attacker;
fall packets increase the fallen unit's damage taken. A seeded direct shot also
asserts the independent numeric total of five laser damage plus a 200-point
ammunition explosion, preserving the shooter attribution through that cascade.
The reactor blast matrix checks every supported chassis, excluded recipients,
callback rollback and restart: the common blast path credits no other unit with
damage inflicted. These checks are in `tests/btech_motion.rs` and
`tests/btech_reactor_explosion.rs`.

`units_killed` now has shared signed storage and field controls for both unit
stores. Material destruction and lethal critical/crew events increment the
attributed attacker's total on their immediate alive-to-destroyed transition.
The increment occurs before nested consequences, so later packets cannot count
the same death again. Self/environmental events have no credited attacker;
overflow rejects the enclosing combat transaction. No last-attacker field or
separate destruction ledger is stored.

`tests/btech_kill_counters.rs` covers all seven attacker/target families, lethal
hull/center-torso damage, signed field bounds, native/Lua agreement, self damage,
restart and callback/overflow rollback. The seeded laser/ammunition composition
also verifies exactly one kill. Mech vacuum exposure now retains the attacking
unit through shared section disablement. Water flooding intentionally remains
self-attributed, including flooding after a shot: the reference's
`mech_flood_section` calls `mech_parts_destroy(mech, mech, ...)`, whereas vacuum's
`mech_location_breach` forwards its attacker. An exposure-triggered fall remains
its own event. The water/vacuum head-exposure matrix verifies both Mech chassis,
surviving head structure, restart and callback rollback.

Vehicle vacuum breaches disable equipment without destroying the unit. The
vehicle branch of `mech_parts_destroy` returns immediately after processing
equipment; its later ground-vehicle death branch is unreachable. A separate
five-family shot matrix requires surviving structure and crew, zero kill credit,
restart, callback rollback and successful nonlethal firing with a full kill
counter. No vehicle mortality change is needed. Exhaustive lethal crew/critical
branch acceptance remains open; this does not
establish complete kill-event parity.

Reactor attribution is checked by `tests/btech_reactor_attribution.rs`: both
shooter stores destroy biped and quad reactors with a direct shot while the
resulting blast destroys a neighboring vehicle. The initiating shot receives
one kill; the self-attributed radial blast adds no further kill to any unit.
Native/Lua, callback rollback and restart agree. The standalone reactor-blast
matrix also requires zero kill credit across all supported recipient families.
The existing material/exposure transition owners account for the initiating
death before detonation; no second counter update was added to the reactor.

Initial vehicle inferno effects now retain the missile attacker's identity.
Under standard fire rules, a fatal heat explosion awards that attacker one kill.
Under advanced fire rules, the first section-fire packets credit damage and any
immediate destruction to the attacker. Later scheduled pulses remain
self-attributed; stationary-unit jelly duration retains its separate behavior.
This follows `mech_heat_effect_apply`, `vehicle_fire_start` and
`vehicle_burn_event`, without storing a last attacker on a burning unit.

`tests/btech_inferno_attribution.rs` covers Mech and vehicle shooters, all five
vehicle families, both fire policies, native/Lua agreement, restart and callback
and counter-overflow rollback. It independently sums initial fire packets,
requires one kill for the seeded standard heat explosion and advanced VTOL
power-plant catastrophe, and advances surviving ground fires through a pulse
after restart to ensure the original shooter receives no further credit.

The damage entry points now account for their owned
packets, but exhaustive attribution acceptance across other physical attacks,
vehicle ammunition cascades and delayed blasts remains work. The physical wrapper
has an internal-damage argument named glancing, but its combat callers pass
zero and apply glancing reduction beforehand. No extra damage was inferred
from that parameter name. This is not a claim of complete damage-statistics or
broader porting parity.
