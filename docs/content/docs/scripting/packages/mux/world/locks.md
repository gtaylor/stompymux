---
title: mux.world.locks
type: docs
weight: -30
toc_hide: false
---

`mux.world.locks` is an immutable namespace of typed `Lock` constants for
[`mux.world.lock_passes`](../lock-passes/). Each constant corresponds directly
to a supported key in an object module's `locks` table. Raw strings are
rejected.

| Constant | Object-module key | Purpose |
| --- | --- | --- |
| `MATCH` | `match` | Prefer an object that passes during key-aware matching. |
| `TRAVERSE` | `traverse` | Traverse an exit. |
| `TAKE` | `take` | Take an object. |
| `USE` | `use` | Use an object. |
| `DROP` | `drop` | Drop an object. |
| `GIVE` | `give` | Give an object. |
| `RECEIVE` | `receive` | Receive a given object. |
| `ENTER` | `enter` | Enter an object, room, BattleTech unit, bay, or hangar. |
| `LEAVE` | `leave` | Leave an object or room. |
| `TELEPORT` | `teleport` | Teleport into a destination. |
| `TELEPORT_OUT` | `teleport_out` | Teleport out of an origin. |
| `LINK` | `link` | Link an exit or object. |
| `SET_HOME` | `set_home` | Set an object's home to a destination. |
| `SPEAK` | `speak` | Speak in a location. |
| `CHANNEL_JOIN` | `channel_join` | Join a channel. |
| `CHANNEL_TRANSMIT` | `channel_transmit` | Transmit on a channel. |
| `CHANNEL_RECEIVE` | `channel_receive` | Receive channel traffic. |
| `IDENTIFY_BUILDING` | `identify_building` | Identify a BattleTech building contact. |

```lua
local passes = mux.world.lock_passes({
  object = exit,
  enactor = ctx.enactor,
  lock = mux.world.locks.TRAVERSE,
})
```

Constants compare by lock identity and stringify as their uppercase name.
Unknown lookups and attempts to modify a constant or the namespace raise
`mux.arg.invalid`.
