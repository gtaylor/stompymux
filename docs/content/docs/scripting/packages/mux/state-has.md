---
title: State:has
type: docs
toc_hide: true
---

Tests whether a persistent state key exists.

## Function

### Synopsis

```lua
state:has( key )
```

### Arguments

`string key`
: A valid state key.

### Returns

`boolean exists`
: Whether the key has a value.

## Notes

An empty string is a present value. Reads observe writes made earlier in the current callback transaction.

## See Also

- [`mux`](../)
- [`State`](../type-state/)
- [`State:get`](../state-get/)
