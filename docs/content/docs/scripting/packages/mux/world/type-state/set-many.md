---
title: "State:set_many"
type: docs
---

Applies several persistent state updates. Use `State:set` or `State:delete` for removals.

Raises `mux.error.codes.object.invalid`, `mux.error.codes.state.invalid`, `mux.error.codes.state.value_too_large`.

## Signature

```lua
State:set_many(values)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `values` | `table<string, StateValue>` |  |

## Returns

No values.

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.state.invalid`
- `mux.error.codes.state.value_too_large`
