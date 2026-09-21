---
title: "mux.world.lock_passes"
type: docs
---

Tests a native object lock without emitting lock messages or performing the
associated action. The lock runs with a silent callback context.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, `mux.error.codes.object.invalid`, or `mux.error.codes.object.unavailable`.

## Signature

```lua
mux.world.lock_passes(options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `options` | `LockPassesOptions` | Lock invocation fields; unknown fields are rejected. |

## Returns

- `boolean passes Whether the selected lock passes.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
