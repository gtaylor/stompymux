---
title: Object Flags
linkTitle: Flags
type: docs
weight: 12
---

Object flags are independent boolean properties on database objects. Native
code stores each flag in a `has_<name>_flag` field and in a matching SQLite
column. There is no packed flag word and no reserved legacy bits.

Flags are displayed after the object type as compact letters. A script or
administrator should use the full flag name when issuing a command because
`ANSI` and `XCODE` both use `X`:

```text
@set <object>=<flag>
@set <object>=!<flag>
```

The `mux` Lua package does not currently expose direct flag getter or setter
functions. Lua logic can invoke an appropriate native command with
`mux.command`; queued commands run as God after the current callback finishes.

## Flag summary

| Flag | Letter | Stored field and column | Native purpose |
| --- | --- | --- | --- |
| `ANSI` | `X` | `has_ansi_flag` | Enables ANSI color and formatting for a player. |
| `ANSIMAP` | `P` | `has_ansimap_flag` | Enables color in BattleTech map displays. |
| `AUDIBLE` | `a` | `has_audible_flag` | Allows sound to propagate through the object or an audible exit. |
| `AUDITORIUM` | `b` | `has_auditorium_flag` | Requires speakers in the location to pass its speech lock. |
| `BLIND` | `(` | `has_blind_flag` | Stored marker with no native consumer; available for game policy. |
| `CONNECTED` | `c` | `has_connected_flag` | Records an active player connection. |
| `DARK` | `D` | `has_dark_flag` | Hides objects from ordinary visibility; see below. |
| `FLOATING` | `F` | `has_floating_flag` | Prevents consistency cleanup from moving an improperly located object home. |
| `GAGGED` | `j` | `has_gagged_flag` | Prevents non-Wizard speech and participates in BattleTech OOC rules. |
| `GOING` | `G` | `has_going_flag` | Marks an object as pending destruction or unavailable for normal processing. |
| `HALTED` | `h` | `has_halted_flag` | Stops queued command dispatch for the object. |
| `IN_CHARACTER` | `#` | `has_in_character_flag` | Marks BattleTech objects and locations as participating in IC play. |
| `LIGHT` | `l` | `has_light_flag` | Makes a non-DARK object visible inside a DARK location. |
| `MONITOR` | `M` | `has_monitor_flag` | Subscribes a player to server monitor broadcasts. |
| `NO_COMMAND` | `n` | `has_no_command_flag` | Excludes commands stored on the object from command matching. |
| `QUIET` | `Q` | `has_quiet_flag` | Suppresses routine confirmations and many movement or management messages. |
| `SAFE` | `s` | `has_safe_flag` | Protects a non-player object from normal destruction. |
| `SUSPECT` | `u` | `has_suspect_flag` | Reports the player's connections and commands to monitoring staff and blocks commands that disallow suspect users. |
| `TRANSPARENT` | `t` | `has_transparent_flag` | Exposes contents through native `look` presentation. |
| `WIZARD` | `W` | `has_wizard_flag` | Grants Wizard status and native Wizard control. |
| `XCODE` | `X` | `has_xcode_flag` | Marks a BattleTech special object managed by XCode. |
| `ZOMBIE` | `z` | `has_zombie_flag` | Marks an inactive BattleTech special object for selected validation and containment behavior. |

## Visibility flags

`DARK`, `LIGHT`, and `TRANSPARENT` affect native presentation. A DARK object is
normally omitted from its location's contents. In a DARK location, only LIGHT
objects that are not also DARK remain visible. DARK only hides a player when
that player is a Wizard. TRANSPARENT changes native `look` output so a location
or thing can expose its contents.

An object's Lua `internal_appearance` or `external_appearance` function can
replace the corresponding native presentation completely. These flags remain
available to native visibility, matching, movement, connection, and channel
logic outside those appearance callbacks.

## Status and lifecycle flags

`CONNECTED` is live connection state maintained by the server. Code should use
it only with player objects; the connected-player APIs are generally a better
interface for Lua.

`GOING` is lifecycle state used while objects are being destroyed and while
database structures are cleaned. Normal logic should treat a GOING object as
unavailable. `FLOATING` changes one consistency-repair behavior, while `SAFE`
protects non-player objects from ordinary destruction. Players are always
treated as safe regardless of their `SAFE` field.

`HALTED` prevents queued commands from running. `NO_COMMAND` instead prevents
the object's attached command scope from participating in command matching;
it does not describe queue state.

## BattleTech flags

`ANSIMAP` is a player display preference. `IN_CHARACTER`, `XCODE`, and
`ZOMBIE` are consulted throughout the BattleTech subsystem. `IN_CHARACTER`
gates combat, damage, movement, economy, and related behavior. `XCODE`
identifies a native special object. `ZOMBIE` marks selected special objects as
inactive.

## Native command restrictions

The normal control check applies before a flag handler runs. Native flag
handlers add these restrictions:

| Flag | Additional restriction |
| --- | --- |
| `CONNECTED` | Only God may change it through the flag command. |
| `DARK` | Only Wizards or God may change it. |
| `GAGGED` | Only Wizards or God may change it. |
| `GOING` | God may set or clear it. An already-GOING non-player object may also have it cleared without this extra restriction. |
| `IN_CHARACTER` | Only Wizards or God may change it. |
| `MONITOR` | Only Wizards or God may change it. |
| `SUSPECT` | Only Wizards or God may change it. |
| `WIZARD` | Only God may change it, and God cannot remove it from themself. |
| `XCODE` | Wizards or God may set it; only God may clear it once set. |
| `ZOMBIE` | Only Wizards or God may change it. |

Flags not listed in this table have no additional flag-specific restriction.
Native control is still role-only: God controls everyone and everything, while
Wizards control non-Wizards but cannot control God, another Wizard, or
themselves.
