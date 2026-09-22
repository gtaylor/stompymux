---
title: "Object:internal_description"
type: docs
linkTitle: "internal_description"
manualLinkTitle: "internal_description"
---

Returns this object's styled internal description, or nil when it is unset.

Raises `mux.error.codes.object.invalid`
for a stale Object.

## Signature

```lua
Object:internal_description()
```

## Parameters

None.

## Returns

- `string? description`

## Related errors

- `mux.error.codes.object.invalid`
