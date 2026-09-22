---
title: "Object:home"
type: docs
linkTitle: "home"
manualLinkTitle: "home"
---

Returns this thing or player's home, or nil when no home is assigned or the
home is being destroyed.

Raises `mux.error.codes.unavailable.checking`
during `@lua/check`, or
`mux.error.codes.object.invalid` when
the receiver is not a thing or player or its stored home is invalid.

## Signature

```lua
Object:home()
```

## Parameters

None.

## Returns

- `Object? home`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.object.invalid`
