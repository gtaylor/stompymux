+++
title = "Lua tactical director"
description = "Plan shared BattleTech objectives from filtered Lua observations"
keywords = ["tactical director", "tactical", "battlefield"]
article_tags = ["battletech"]
+++

# Lua tactical director

`require("tactical_director")` provides a pure, deterministic policy for
turning a filtered `btech.tactical.observe` snapshot into attack-move
intentions. It preserves matching orders, reports arrivals, and leaves paused
or blocked controllers for the caller to handle. Submit its intentions with
`btech.tactical.submit`, which validates the whole batch before committing it.

The opt-in `require("tactical_encounter_example")` package demonstrates the
workflow. Call `create`, then `start` to explicitly attach, configure, and
resume the units you own, and call `tick` once per simulation tick. It evaluates
the director at most once per three seconds of simulation time and re-observes
once when a revision conflict requires a retry. If a selected member is
detached, unplaced, or transferred,
the example reports it as unavailable and waits without submitting a partial
roster. Requiring either package schedules no callbacks and changes no units.

For example, from your scenario script, replace these IDs and coordinates with
your assigned friendly units and battlefield objective:

```lua
local btech = require("btech")
local example = require("tactical_encounter_example")
local encounter = example.create(
  { 101, 102 },
  { map = 100, x = 30, y = 20, arrival_radius = 2 },
  { config = { fire_mode = btech.autopilot.fire_modes.OPPORTUNISTIC } }
)
example.start(encounter)

-- Invoke from your scenario's simulation callback.
local result = example.tick(encounter)
```

Use units without existing attached controllers for `start`. Keep the encounter
state in your scenario and inspect `result.state.report` on evaluated passes
for preserved intentions, arrivals, skipped controllers, and unavailable
members. A successful manual control pauses its unit; the director preserves
that pause. Resume it explicitly through `btech.autopilot.resume` when desired.
