---
title: "Object:set_internal_description"
type: docs
---

Sets this object's styled internal description. Nil or an empty string clears it.

Raises `mux.error.codes.unavailable.checking`
during `@lua/check`, `mux.error.codes.object.invalid`
for a stale Object,
`mux.error.codes.object.unavailable`
when the object is being destroyed, or
`mux.error.codes.arg.invalid` for text
that is too long or has invalid UTF-8 or styled-text markup.

## Signature

```lua
Object:set_internal_description(description)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `description` | `string\|nil` | Valid UTF-8 styled-text markup without embedded NUL bytes, or nil to clear the internal description. This argument must be supplied explicitly. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
- `mux.error.codes.arg.invalid`
