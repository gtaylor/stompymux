---
title: entries
type: docs
toc_hide: false
---

Returns every supported native attribute and its current value.

## Function

### Synopsis

```lua
attributes:entries( )
```

### Arguments

None.

### Returns

`table entries`
: A name-to-string table of all supported attributes.

## Notes

Unset attributes are included with an empty-string value.

## See Also

- [`mux`](../../../)
- [`Attribute`](../)
- [`Attribute:get`](../get/)
