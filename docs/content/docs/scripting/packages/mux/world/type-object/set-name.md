---
title: "Object:set_name"
type: docs
---

Changes this object's name using native object-name validation.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, `mux.error.codes.object.invalid`, or `mux.error.codes.object.unavailable`.

## Signature

```lua
Object:set_name(name)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `name` | `string` | New UTF-8 name, optionally containing styled-text markup. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
