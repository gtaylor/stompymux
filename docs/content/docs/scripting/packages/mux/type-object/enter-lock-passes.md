---
title: enter_lock_passes
type: docs
toc_hide: false
---

Tests whether an enactor passes this exit's default traversal lock.

## Function

### Synopsis

```lua
exit:enter_lock_passes( enactor )
```

### Arguments

`number or Object enactor`
: The object attempting traversal.

### Returns

`boolean passes`
: Whether the lock passes.

## Notes

The receiver must be an exit. The check sends no lock messages and does not move the enactor. This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../../)
- [`Object`](../)
