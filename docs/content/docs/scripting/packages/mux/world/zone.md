---
title: zone
---

Returns an object's assigned zone.

```lua
local zone = mux.world.zone(object)
```

## Parameters

`number|Object object`
: The live database object to inspect.

## Returns

`Object|nil zone`
: A handle for the assigned zone, or `nil` when the object has no zone.

## Errors and availability

Raises `mux.object.invalid` for an invalid object reference or an invalid zone
stored on the object. This function is unavailable during `@lua/check` and
raises `mux.unavailable.checking` there.
