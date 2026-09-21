---
title: BattleTech unit administration
weight: 29
description: Input and target guarantees for wizard-only BattleTech unit commands
---

The wizard-only `@mech` command administers the BattleTech unit containing the
wizard. Its switches validate their complete input before changing that unit.

`@mech/setarmor` accepts a location and zero to three supplied values. Each
value must be an integer from 0 through 255, and rear armor is restricted to
locations that support it. A rejected command leaves every armor field
unchanged.

`@mech/repair` validates the exact argument count for its selected repair type
and rejects unknown types before applying a repair. `@mech/restock` only
accepts a live ammunition critical slot. `@mech/delinftech` applies only to
battle armor.

Speed switches reject negative and non-finite values. Integer switches validate
their ranges rather than accepting out-of-range input. These rules require no
server configuration.
