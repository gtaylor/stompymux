---
title: "mux.world.destroy_object"
type: docs
---

Silently schedules a live object for destruction by the normal maintenance purge.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, `mux.error.codes.object.invalid`, `mux.error.codes.object.unavailable`, or `mux.error.codes.internal` for an unexpected native destruction result.

## Signature

```lua
mux.world.destroy_object(object, options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `object` | `DbRef\|Object` | Object to destroy. |
| `options?` | `DestroyOptions` | Destruction controls; unknown fields are rejected. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
- `mux.error.codes.internal`
