---
title: "Object:description"
type: docs
linkTitle: "description"
manualLinkTitle: "description"
---

Returns this object's styled description, or nil when it is unset.

Raises `mux.error.codes.object.invalid`
for a stale Object.

## Signature

```lua
Object:description()
```

## Parameters

None.

## Returns

- `string? description`

## Related errors

- `mux.error.codes.object.invalid`
