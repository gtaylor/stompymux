---
title: get
type: docs
toc_hide: false
---

Gets a supported native attribute.

## Function

### Synopsis

```lua
attributes:get( name )
```

### Arguments

`string name`
: A supported native attribute name.

### Returns

`string or nil value`
: The raw value, or `nil` when unset.

## Notes

Unsupported names and invalid handles raise a Lua error.

The removed `Object.description` and `Object.inside_description` properties are
available as `attributes:get("Description")` and
`attributes:get("InternalDescription")`, respectively. Both return `nil` when
unset.

## See Also

- [`mux`](../../../)
- [`Attribute`](../)
- [`Attribute:set`](../set/)
