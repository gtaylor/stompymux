---
title: "State:set"
type: docs
---

Sets a supported value, or deletes the key when `value` is nil.
The `value` argument is required; omission raises
`mux.error.codes.state.invalid`.

Raises invalid-object/key/value errors or `mux.error.codes.state.value_too_large`.

## Signature

```lua
State:set(key, value)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `key` | `string` |  |
| `value` | `StateValue\|nil` |  |

## Returns

No values.

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.state.invalid`
- `mux.error.codes.state.value_too_large`
