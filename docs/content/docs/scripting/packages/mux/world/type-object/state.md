---
title: "Object:state"
type: docs
---

Creates a persistent-state handle for an exact, case-sensitive namespace.

Raises `mux.error.codes.object.invalid`, `mux.error.codes.unavailable.checking`, `mux.error.codes.state.invalid`.

## Signature

```lua
Object:state(namespace)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `namespace` | `string` |  |

## Returns

- `State state`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.unavailable.checking`
- `mux.error.codes.state.invalid`
