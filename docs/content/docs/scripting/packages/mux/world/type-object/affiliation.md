---
title: affiliation
---

Returns an object's assigned affiliation.

```lua
local affiliation = object:affiliation()
```

The receiver is the live database object to inspect.

## Returns

`Object|nil affiliation`
: A handle for the assigned object, or `nil` when the object has no
  affiliation or its affiliate is being destroyed.

Affiliations are persistent object references only. They do not affect command
matching, events, permissions, or other server behavior.

## Errors and availability

Raises `mux.object.invalid` for an invalid receiver or an invalid affiliation
stored on the object. This method is unavailable during
`@lua/check` and raises `mux.unavailable.checking` there.
