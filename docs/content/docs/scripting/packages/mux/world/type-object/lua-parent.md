---
title: "Object:lua_parent"
type: docs
linkTitle: "lua_parent"
manualLinkTitle: "lua_parent"
---

Returns this object's direct Lua parent path, or nil when none is assigned.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.object.invalid`.

## Signature

```lua
Object:lua_parent()
```

## Parameters

None.

## Returns

- `string? parent `object_logic`-relative parent path.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.object.invalid`
