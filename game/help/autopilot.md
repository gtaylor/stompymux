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

A successful manual movement or combat control pauses automation. Resume it
explicitly after the player is done. Radio and cockpit command interfaces do
not manage this subsystem.

For group objectives, `btech.tactical.observe(units, feedback_cursors)` returns a
filtered friendly-force snapshot, and `btech.tactical.submit(intentions)` applies
revision-guarded unit orders atomically. Shared sightings retain their observer;
an attack still requires the receiving unit's own acquired hostile contact.
See [Lua tactical director](help:tactical_director) for the opt-in scripted example.
