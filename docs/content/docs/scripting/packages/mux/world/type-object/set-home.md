---
title: "Object:set_home"
type: docs
---

Sets this thing or player's home to a live object capable of containing
objects.

Raises `mux.error.codes.unavailable.checking`
during `@lua/check`, `mux.error.codes.arg.invalid`
when `new_home` is omitted,
`mux.error.codes.object.invalid` when
the receiver is not a thing or player, the home cannot contain objects, or
the object would be its own home, or
`mux.error.codes.object.unavailable`
when the receiver or home is being destroyed.

## Signature

```lua
Object:set_home(new_home)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `new_home` | `DbRef\|Object` | Live room, thing, or player to assign as the object's home. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
