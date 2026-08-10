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

Supported movement goals are `chasetarget`, `dumbfollow`, `dumbgoto`,
`enterbase`, `follow`, `goto`, `leavebase`, `oldgoto`, and `roam`. Supported
immediate orders are `autogun`, `dropoff`, `embark`, `pickup`, `shutdown`,
`speed`, `startup`, and `udisembark`.

Autogun selects working, recycled weapons that can engage the target without
exceeding its heat limit. Weapons that require ammunition are skipped when no
compatible rounds remain. It does not reserve scarce ammunition. Automatic
sensors choose among visual, light-amplification, infrared, electromagnetic,
radar, and probe sensors based on visibility and the current target. A manual
sensor selection disables automatic changes until automatic sensor judgment is
enabled again.
