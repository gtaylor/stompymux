---
title: "mux.config.get"
type: docs
---

Returns the live scalar value of an exact, case-sensitive configuration directive.

Raises `mux.error.codes.arg.invalid`, `mux.error.codes.config.not_found`, `mux.error.codes.config.unsupported`, or `mux.error.codes.internal`.

## Signature

```lua
mux.config.get(name)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `name` | `string` | Configuration directive name; embedded NUL bytes are rejected. |

## Returns

- `ConfigValue value Current value represented by its native Lua scalar type.`

## Related errors

- `mux.error.codes.arg.invalid`
- `mux.error.codes.config.not_found`
- `mux.error.codes.config.unsupported`
- `mux.error.codes.internal`
