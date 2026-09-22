---
title: BattleTech unit administration
weight: 29
description: How Rust and Lua administer BattleTech units
---

BattleTech unit state is owned by the Rust `World` and `BtechState`. The
`src/btech/` administrative operations validate target objects, sections,
values, and the resulting unit state. Lua bindings in
`src/lua/packages/btech/` expose those operations to trusted game code.

For example, `btech.unit.set_armor(unit, section, patch)` accepts a section
constant and a table containing one or more of `armor`, `internal`, and
`rear_armor`. Supplied values must be integers from 0 through 255; the
section must exist on the unit, and rear armor is limited to sections that
support it. The binding checks the complete patch before applying it.

`btech.unit.restock_ammunition(unit, section, slot)` validates a live
ammunition critical slot. Other administrative operations, including repair
and movement settings, use typed arguments and reject unsupported targets or
out-of-range values. Failed validation leaves the candidate world unchanged;
a persistence failure rolls the operation back before its effects are sent.

Wizard-facing native management is provided through `@btech` commands for
registration, asset loading, placement, map conditions, and inspection. The
Rust command registry supplies admission and dispatch. The Lua package
reference lists each available unit operation and its arguments.
