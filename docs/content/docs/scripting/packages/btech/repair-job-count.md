---
title: repair_job_count
type: docs
toc_hide: false
---

Returns the number of pending repair jobs on a live unit.

## Function

### Synopsis

```lua
btech.repair_job_count( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
