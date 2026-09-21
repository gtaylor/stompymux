---
title: "Object:location"
type: docs
---

Returns this thing or player's current location, or nil when no location is
assigned or the location is being destroyed.

Raises `mux.error.codes.unavailable.checking`
during `@lua/check`, or
`mux.error.codes.object.invalid` when
the receiver is not a thing or player or its stored location is invalid.

## Signature

```lua
Object:location()
```

## Parameters

None.

## Returns

- `Object? location`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.object.invalid`
