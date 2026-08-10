---
title: btech.tech_time
type: docs
toc_hide: true
---

Runs the legacy technician-time query.

## Function

### Synopsis

```lua
btech.tech_time(  )
```

### Arguments

None.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The legacy handler sends a formatted time directly to its invocation player and does not write a return value; through the Lua adapter the numeric result is therefore `0`.

## See Also

- [`btech`](../)
