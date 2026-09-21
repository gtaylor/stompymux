---
title: "Object:destination"
type: docs
---

Returns this exit's destination, or nil when it is unlinked or the
destination is being destroyed.

Raises `mux.error.codes.unavailable.checking`
during `@lua/check`, or
`mux.error.codes.object.invalid` when
the receiver is not an exit or its stored destination is invalid.

## Signature

```lua
Object:destination()
```

## Parameters

None.

## Returns

- `Object? destination`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.object.invalid`
