---
title: "mux.world.teleport_object"
type: docs
---

Teleports a thing or player through the native movement path.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, `mux.error.codes.object.invalid`, or `mux.error.codes.object.unavailable`.

## Signature

```lua
mux.world.teleport_object(options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `options` | `TeleportOptions` | Teleport fields; unknown fields are rejected. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
