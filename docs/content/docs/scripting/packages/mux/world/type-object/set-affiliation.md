---
title: "Object:set_affiliation"
type: docs
linkTitle: "set_affiliation"
manualLinkTitle: "set_affiliation"
---

Assigns this object's affiliation, or clears it when `affiliation` is nil.

Raises `mux.error.codes.unavailable.checking` during `@lua/check`, `mux.error.codes.arg.invalid` when `affiliation` is omitted, `mux.error.codes.object.invalid` for an invalid reference, or `mux.error.codes.object.unavailable` when either object is being destroyed.

## Signature

```lua
Object:set_affiliation(affiliation)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `affiliation` | `DbRef\|Object\|nil` | Any live object to assign, or nil to clear the affiliation. This argument must be supplied explicitly. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
