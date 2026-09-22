---
title: "Object:set_zone"
type: docs
linkTitle: "set_zone"
manualLinkTitle: "set_zone"
---

Assigns this object's zone, or clears it when `zone` is nil.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid` when `zone` is omitted, `mux.error.codes.object.invalid`, or `mux.error.codes.object.unavailable`.

## Signature

```lua
Object:set_zone(zone)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `zone` | `DbRef\|Object\|nil` | Live thing or room to assign, or nil to clear the zone. This argument must be supplied explicitly. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
