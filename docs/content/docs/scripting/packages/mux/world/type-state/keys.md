---
title: "State:keys"
type: docs
linkTitle: "keys"
manualLinkTitle: "keys"
---

Lists keys sorted in native key order.

Raises `mux.error.codes.state.unavailable` outside a callback transaction or if state changes while enumerating.

## Signature

```lua
State:keys()
```

## Parameters

None.

## Returns

- `string[] keys`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.state.unavailable`
