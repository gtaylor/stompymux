---
title: zone
---

Returns an object's assigned zone.

```lua
local zone = object:zone()
```

The receiver is the live database object to inspect.

## Returns

`Object|nil zone`
: A handle for the assigned zone, or `nil` when the object has no zone or its
  zone is being destroyed.

## Errors and availability

Raises `mux.object.invalid` for an invalid receiver or an invalid zone stored
on the object. This method is unavailable during `@lua/check` and
raises `mux.unavailable.checking` there.
