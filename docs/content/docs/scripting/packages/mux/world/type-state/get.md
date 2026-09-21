---
title: "State:get"
type: docs
---

Gets a stored value, an optional default, or nil.

Raises `mux.error.codes.object.invalid` or `mux.error.codes.state.invalid`.

## Signature

```lua
State:get(key, default)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `key` | `string` |  |
| `default?` | `T` |  |

## Returns

- `StateValue|T|nil value`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.state.invalid`
