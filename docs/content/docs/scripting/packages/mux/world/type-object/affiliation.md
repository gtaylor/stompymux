---
title: "Object:affiliation"
type: docs
linkTitle: "affiliation"
manualLinkTitle: "affiliation"
---

Returns this object's assigned affiliation, or nil when none is assigned or the affiliate is being destroyed.

Raises `mux.error.codes.unavailable.checking` during `@lua/check`, or `mux.error.codes.object.invalid` for an invalid receiver or stored affiliation.

## Signature

```lua
Object:affiliation()
```

## Parameters

None.

## Returns

- `Object? affiliation Assigned affiliation.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.object.invalid`
