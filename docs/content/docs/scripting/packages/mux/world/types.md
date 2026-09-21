---
title: "mux.world.types"
type: docs
---

Immutable typed constants available in the Stompymux-rs Lua runtime.

| Constant | Type or native code | Description |
| --- | --- | --- |
| `mux.world.types.ROOM` | `RoomObjectType` | Detached room kind accepted by `mux.world.create_object`. |
| `mux.world.types.THING` | `ThingObjectType` | Contained thing kind accepted by `mux.world.create_object`. |
| `mux.world.types.EXIT` | `ExitObjectType` | Attached exit kind accepted by `mux.world.create_object`. |
| `mux.world.types.PLAYER` | `PlayerObjectType` | Player kind; existing players may have this type, but scripts cannot create them. |
