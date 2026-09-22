---
title: "Powers:add"
type: docs
linkTitle: "add"
manualLinkTitle: "add"
---

Grants a power and reports whether the object changed.

Raises `mux.error.codes.object.invalid`, `mux.error.codes.unavailable.checking`, or `mux.error.codes.power.invalid`.

## Signature

```lua
Powers:add(power)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `power` | `Power` | Checked constant from `mux.world.powers`. |

## Returns

- `boolean changed`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.unavailable.checking`
- `mux.error.codes.power.invalid`
