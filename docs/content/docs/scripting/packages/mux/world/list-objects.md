---
title: "mux.world.list_objects"
type: docs
---

Lists database objects matching optional type and direct-zone filters.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, `mux.error.codes.object.invalid`, or `mux.error.codes.object.unavailable`.

## Signature

```lua
mux.world.list_objects(options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `options?` | `ListObjectsOptions` | Optional filters; unknown fields are rejected. |

## Returns

- `Object[] objects Matching objects in ascending dbref order.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
