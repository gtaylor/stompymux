---
title: "Object:set_destination"
type: docs
linkTitle: "set_destination"
manualLinkTitle: "set_destination"
---

Sets this exit's destination, or clears it when `destination` is nil.

Raises `mux.error.codes.unavailable.checking` during `@lua/check`, `mux.error.codes.arg.invalid` when `destination` is omitted, `mux.error.codes.object.invalid` when the receiver is not an exit or the destination cannot contain objects, or `mux.error.codes.object.unavailable` when the receiver or destination is being destroyed.

## Signature

```lua
Object:set_destination(destination)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `destination` | `DbRef\|Object\|nil` | Live object capable of containing objects, or nil to unlink this exit. This argument must be supplied explicitly. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
