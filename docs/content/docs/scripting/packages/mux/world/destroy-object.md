---
title: destroy_object
type: docs
---

Silently schedules a live object for destruction by the normal maintenance
purge. The object remains accessible, with the same handle generation, until
that purge runs.

## Function

```lua
mux.world.destroy_object(object)
mux.world.destroy_object(object, { override = true })
```

The optional Boolean `override` field bypasses a target's `SAFE` flag. It does
not bypass protection for God, database object `#0`, configured start/default
homes, or Wizard players. Rooms, things, exits, and non-Wizard players are
supported. Repeated destruction requests are rejected.

The operation performs native XCODE cleanup and records God as the destroyer of
a player, but emits no crumble messages. It raises `mux.object.invalid` for an
invalid reference, `mux.object.unavailable` for a protected or already-going
object, `mux.arg.invalid` for invalid options, and
`mux.unavailable.checking` during `@lua/check`.

## Example

```lua
local object = mux.world.create_object({ type = mux.world.types.THING, name = "Temporary", location = room })
mux.world.destroy_object(object)
```
