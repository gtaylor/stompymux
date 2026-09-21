---
title: "State:delete"
type: docs
---

Deletes a state key and reports whether it existed.

Raises `mux.error.codes.object.invalid`, `mux.error.codes.state.invalid`.

## Signature

```lua
State:delete(key)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `key` | `string` |  |

## Returns

- `boolean existed`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.state.invalid`
