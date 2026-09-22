---
title: Object Powers
linkTitle: Powers
description: A reference of in-game object powers
type: docs
weight: 13
---

Rust `Object` values hold powers in a typed `PowerSet`. They are separate
from [object flags](/docs/scripting/flags/): flags describe object state and presentation,
while powers grant an exception to normal server behavior. SQLite stores the
current power in a `has_idle_power` column.

`IDLE` is the complete set of powers currently registered by the MUX server.
Historical MUX and BattleTech power names are not accepted.

## Power summary

| Power | SQLite column | Native purpose |
| --- | --- | --- |
| `IDLE` | `has_idle_power` | Exempts a connected player from the inactivity timeout. |

## IDLE

The `IDLE` power prevents the maintenance timer from disconnecting a connected
player when the descriptor's inactivity timeout expires. It is meaningful only
on a player with an active connection; setting it on another object type has no
native effect.

Wizards and God receive the same inactivity-timeout exemption implicitly,
whether or not their `has_idle_power` field is set. `IDLE` does not mark a
connection active, change the interval at which idle connections are checked,
or exempt an unauthenticated connection from the login timeout.

The relevant `[mux]` configuration values are `idle_interval`, which controls
how often the server checks connected players, and `idle_timeout`, which is the
default per-connection inactivity limit.

## Managing powers

Only Wizards and God may use `@power`. The normal control check also applies to
the target:

```text
@power <object>=<power>
@power <object>=!<power>
```

The first form grants a power and the second removes it. Power names are
case-insensitive for `@power`.

Wizards can use the following native commands to discover and inspect powers:

```text
@list powers
@examine <object>
@search power=idle
```

`@list powers` displays every registered power. `@examine` includes a
`Powers:` line for the target. `@search power=idle` finds objects with the
stored `IDLE` power; its power name is case-insensitive, just like `@power`.

Lua logic uses typed constants such as `mux.world.powers.IDLE` with an
[`Object:powers`](/docs/scripting/packages/mux/world/type-object/powers/) collection. Raw
power-name strings are intentionally not accepted. Lua changes use God
authority and participate in the callback's world transaction. A failed
callback or save rolls them back.
