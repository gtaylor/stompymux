---
title: Repair-facility command validation
weight: 29
description: Input and target guarantees for BattleTech mechrep administration
---

BattleTech repair-facility (mechrep) commands validate their complete input
before mutating the selected unit. `SETARMOR` accepts a location and zero to
three supplied values (one to four arguments total); each value is an integer
from 0 through 255, and rear armor is restricted to torso locations. A rejected
command leaves every armor field unchanged.

`REPAIR` validates the exact argument count for its chosen type and rejects an
unknown type before applying a repair. `RESTOCK` only accepts a live ammunition
critical slot. `DELINFTECH` is a battlesuit-only operation and uses the normal
repair-facility authorization and selected-target context.

Speed setters reject negative and non-finite values. Integer setters validate
their ranges rather than accepting out-of-range input. These are command-level
rules; no `stompymux.toml` setting is required or affected.
