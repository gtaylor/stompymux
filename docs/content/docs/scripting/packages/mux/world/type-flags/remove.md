---
title: "Flags:remove"
type: docs
---

Removes a flag and reports whether the object changed.

Raises `mux.error.codes.object.invalid`, `mux.error.codes.unavailable.checking`, `mux.error.codes.flag.invalid`, or `mux.error.codes.object.unavailable`.

## Signature

```lua
Flags:remove(flag)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `flag` | `Flag` | Checked constant from `mux.world.flags`. |

## Returns

- `boolean changed`

## Related errors

- `mux.error.codes.object.invalid`
- `mux.error.codes.unavailable.checking`
- `mux.error.codes.flag.invalid`
- `mux.error.codes.object.unavailable`
