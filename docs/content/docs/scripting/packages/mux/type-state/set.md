---
title: set
type: docs
toc_hide: false
---

Sets or deletes a persistent state value.

## Function

### Synopsis

```lua
state:set( key, value )
```

### Arguments

`string key`
: A valid state key.

`string, boolean, finite number, or nil value`
: The new value; `nil` deletes the key.

### Returns

Nothing.

## Notes

The update belongs to the current callback transaction. Invalid types, non-finite numbers, and configured storage-limit violations raise an error.

## See Also

- [`mux`](../../)
- [`State`](../)
- [`State:delete`](../delete/)
- [`State:set_many`](../set-many/)
