+++
title = "Mech repair-facility commands"
keywords = ["mechrep", "setarmor", "repair", "restock", "delinftech", "setmaxspeed", "setjumpspeed"]
article_tags = ["wizard_commands"]
description = "Validated administrative commands for a selected BattleTech unit"
wizard_only = true
+++

# Mech repair-facility commands

These commands operate on the repair facility's selected target. They reject
invalid input before changing the unit.

`SETARMOR <location> [front] [internal] [rear]` accepts one through four
arguments total. Each supplied numeric value must be an integer from `0` to
`255`; rear armor is valid only for a torso. Invalid counts, values, or rear
locations leave all armor values unchanged.

`REPAIR <location> <type> [value]` accepts `ARMOR`, `INTERNAL`, `REAR`, or
`CRIT` with a value, and `SECTION` without one. Unknown types and wrong arity
are rejected before any repair is applied.

`RESTOCK <location> <critical-slot>` only refills a live ammunition critical.
It rejects non-ammunition and destroyed slots.

`DELINFTECH` requires a selected battlesuit target; it is evaluated through the
same repair-facility authorization and target checks as other mechrep commands.

`SETMAXSPEED <MP>` and `SETJUMPSPEED <MP>` require finite, non-negative
numbers. The integer setters likewise reject values outside their documented
ranges rather than clamping or partially applying them.
