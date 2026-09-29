+++
title = "Autopilot"
description = "Direct automated BattleTech units from Lua"
keywords = ["autopilot", "lua", "orders", "navigation"]
article_tags = ["battletech"]
+++

# Autopilot

Trusted in-game Lua scripts manage one autopilot controller per Mech or ground
vehicle through `btech.autopilot`. Attach a controller, submit typed orders, and
resume it to begin execution. The controller retains its orders across restarts.

The initial orders are move, hold, follow, patrol, attack, and attack-move.
Use `btech.autopilot.status(unit)` to inspect the queue and execution state,
`observe(unit)` for the unit's sensor-limited battlefield view, and
`feedback(unit)` for order outcomes. Fire control starts in weapons-hold mode;
Lua must enable assigned-target or opportunistic fire explicitly.

A unit in an active C3 or C3i network also sees what its available peers see.
Such observation contacts are marked `relayed`, and every contact gains the
network's shared `network_range`. Target choice scores ranges with the shared
distance, as the shot itself will, but always prefers an enemy the unit holds
itself; a relayed enemy is chosen only when nothing else is in view. The unit
then closes on and faces it, looking for a clear line, but does not lock or fire
until its own sensors acquire it. Attack orders may name a relayed enemy.

A successful manual movement or combat control pauses automation. Resume it
explicitly after the player is done. Radio and cockpit command interfaces do
not manage this subsystem.

For group objectives, `btech.tactical.observe(units, feedback_cursors)` returns a
filtered friendly-force snapshot, and `btech.tactical.submit(intentions)` applies
revision-guarded unit orders atomically. Shared sightings retain their observer
and mark whether that observer holds them only through its network; an attack
requires a hostile contact held by the receiving unit or its C3/C3i network.
See [Lua tactical director](help:tactical_director) for the opt-in scripted example.
