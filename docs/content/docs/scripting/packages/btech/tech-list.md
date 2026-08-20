---
title: tech_list
type: docs
toc_hide: false
---

Lists the parts needed to repair a live unit.

## Function

### Synopsis

```lua
btech.tech_list( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.tech_list_ref`](../tech-list-ref/)
