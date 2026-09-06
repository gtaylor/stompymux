+++
title = "Autopilot"
description = "Queue movement and combat orders for an automated BattleTech unit"
keywords = ["autopilot", "autogun", "addcommand", "delcommand", "listcommands"]
article_tags = ["battletech"]
+++

# Autopilot

An autopilot executes supported orders in queue order. Use `ADDCOMMAND` to add
an order, `LISTCOMMANDS` to inspect the queue, `DELCOMMAND <number>` to remove
one order, and `DELCOMMAND -1` to clear the queue. A queue holds at most 100
orders. Commands that are recognized but not implemented are rejected instead
of being left in the queue.

`ENGAGE` requires the autopilot object to be physically installed in a
registered unit and starts its queued activity. `DISENGAGE` stops activity but
preserves both the unit association and the queue. While engaged, the first
queued order is active: it cannot be deleted, and a nonempty queue cannot be
cleared. Orders may still be appended and later pending orders removed. These
rules are shared by the legacy commands and the typed `btech.autopilot` Lua API.
Legacy radio `autogun on` and `target` commands re-engage a disengaged autopilot
when its preserved installation and association remain valid; failure leaves
its firing state unchanged.

Supported movement goals are `chasetarget`, `dumbfollow`, `dumbgoto`,
`enterbase`, `follow`, `goto`, `leavebase`, `oldgoto`, and `roam`. Supported
immediate orders are `autogun`, `dropoff`, `embark`, `pickup`, `shutdown`,
`speed`, `startup`, and `udisembark`.

Lua scripts can use `btech.autopilot.status`, `add_order`, `remove_order`,
`clear_orders`, `engage`, `disengage`, and `event_stats`. Typed orders reject
unknown fields and invalid values before changing the queue. Coordinates,
headings, percentages, directions, modes, and registered unit targets are
validated immediately; mutable map, terrain, damage, carrier, and startup
conditions are checked when the order executes.

Autogun selects working, recycled weapons that can engage the target without
exceeding its heat limit. Weapons that require ammunition are skipped when no
compatible rounds remain. It does not reserve scarce ammunition. Automatic
sensors choose among visual, light-amplification, infrared, electromagnetic,
radar, and probe sensors based on visibility and the current target. A manual
sensor selection disables automatic changes until automatic sensor judgment is
enabled again.
