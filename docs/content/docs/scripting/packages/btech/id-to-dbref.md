---
title: id_to_dbref
type: docs
toc_hide: false
---

Resolves a two-character tactical ID on a unit's map.

## Function

### Synopsis

```lua
btech.id_to_dbref( unit_or_map, id )
```

### Arguments

`number unit_or_map`
: The observing unit or map dbref.

`string id`
: The two-character tactical ID.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The legacy handler formats a successful dbref with a leading `#`; the numeric Lua adapter currently converts that format to `0`.

## See Also

- [`btech`](../)
