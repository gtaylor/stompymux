---
title: Object Powers
linkTitle: Powers
description: A reference of in-game object powers
type: docs
weight: 13
---

Object powers are independent boolean privileges stored on database objects.
They are separate from [object flags](../flags/): flags describe object state
and presentation, while powers grant an exception to normal server behavior.
Native code stores each power in a `has_<name>_power` field and in a matching
SQLite column.

`IDLE` is the complete set of powers currently registered by the MUX server.
Historical MUX and BattleTech power names are not accepted.

## Power summary

| Power | Stored field and column | Native purpose |
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

The `mux` Lua package does not currently expose direct power getter or setter
functions. Privileged Lua logic can queue the native `@power` command with
`mux.command`; queued commands run as God after the current callback finishes.
