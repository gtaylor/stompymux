---
title: "State:get_many"
type: docs
linkTitle: "get_many"
manualLinkTitle: "get_many"
---

Returns only the requested keys that are present.

Raises `mux.error.codes.object.invalid`, `mux.error.codes.state.invalid`.

## Signature

```lua
State:get_many(keys)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `keys` | `string[]` |  |

## Returns

- `table<string, StateValue> values`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.state.invalid`
