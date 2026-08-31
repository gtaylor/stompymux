---
title: mux.world.lock_passes
type: docs
weight: -40
toc_hide: false
---

Tests a native object lock without emitting lock messages or performing the
associated action.

## Function

### Synopsis

```lua
mux.world.lock_passes({
  object = object,
  enactor = enactor,
  lock = mux.world.locks.TRAVERSE,
  cause = cause,
  subject = subject,
})
```

### Arguments

`number or Object object`
: The object whose lock is tested.

`number or Object enactor`
: The object attempting the action.

`Lock lock`
: A typed [`mux.world.locks`](../locks/) constant identifying the lock to test.

`number or Object cause` (optional)
: The object that caused the action. Defaults to `enactor`.

`number or Object subject` (optional)
: The object acted upon within the lock context. Defaults to `enactor`.

### Returns

`boolean passes`
: Whether the selected lock passes.

## Notes

The function is unavailable during `@lua/check`. It evaluates the same Lua
lock callback as the corresponding native action, but always sets `ctx.silent`
to true and discards any lock messages returned by the callback.

## See Also

- [`mux.world.locks`](../locks/)
- [`mux.world`](../)
