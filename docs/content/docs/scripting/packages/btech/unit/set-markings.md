---
title: set_markings
type: docs
toc_hide: false
---

Sets or clears a live unit's markings.

## Function

### Synopsis

```lua
btech.unit.set_markings( unit, markings )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`string|nil markings`
: The markings as a non-empty string of at most 16,383 bytes, or `nil` to clear
  them.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
