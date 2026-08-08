---
title: State:delete
type: docs
toc_hide: true
---

Deletes a persistent state value.

## Function

### Synopsis

```lua
state:delete( key )
```

### Arguments

`string key`
: A valid state key.

### Returns

`boolean existed`
: Whether the key existed before deletion.

## Notes

The deletion belongs to the current callback transaction.

## See Also

- [`mux`](../)
- [`State`](../type-state/)
- [`State:set`](../state-set/)
