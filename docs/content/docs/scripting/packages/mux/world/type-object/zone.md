---
title: "Object:zone"
type: docs
---

Returns this object's assigned zone, or nil when no zone is assigned or the zone is being destroyed.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.object.invalid`.

## Signature

```lua
Object:zone()
```

## Parameters

None.

## Returns

- `Object? zone Assigned zone.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.object.invalid`
