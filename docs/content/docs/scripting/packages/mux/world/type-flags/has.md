---
title: "Flags:has"
type: docs
---

Tests whether this object has a flag.

Raises `mux.error.codes.object.invalid` or `mux.error.codes.flag.invalid`.

## Signature

```lua
Flags:has(flag)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `flag` | `Flag` | Checked constant from `mux.world.flags`. |

## Returns

- `boolean present`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.flag.invalid`
