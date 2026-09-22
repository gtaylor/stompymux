---
title: "Powers:has"
type: docs
linkTitle: "has"
manualLinkTitle: "has"
---

Tests whether this object has a power.

Raises `mux.error.codes.object.invalid` or `mux.error.codes.power.invalid`.

## Signature

```lua
Powers:has(power)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `power` | `Power` | Checked constant from `mux.world.powers`. |

## Returns

- `boolean present`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.power.invalid`
