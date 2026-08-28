---
title: tech_time
type: docs
toc_hide: false
---

Returns a player's remaining technician time in seconds.

## Function

### Synopsis

```lua
btech.repair.tech_time( player )
```

### Arguments

`number player`
: The player dbref.

### Returns

`number seconds`
: The remaining technician time in seconds, or `0` when the player has no pending technician time.

## Notes

This function is available only in a running Lua callback. Invalid players, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
