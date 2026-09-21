---
title: "mux.world.create_object"
type: docs
---

Creates a room, thing, or exit selected by a typed object-kind constant.
Rooms are detached; things require a container and receive a home; exits
require a source and may be linked to a destination. Unknown fields and
fields that do not apply to the selected type are rejected.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, `mux.error.codes.object.invalid`, `mux.error.codes.object.unavailable`, or `mux.error.codes.internal` if a validated object kind reaches an unsupported native creation branch.

## Signature

```lua
mux.world.create_object(options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `options` | `CreateObjectOptions` | Exact creation fields selected by `options.type`. |

## Returns

- `Object object Newly created object.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
- `mux.error.codes.internal`
