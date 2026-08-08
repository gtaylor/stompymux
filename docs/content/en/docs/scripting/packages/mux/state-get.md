---
title: State:get
linkTitle: State:get
type: docs
weight: 223
---

# `State:get`

Gets a persistent state value.

## Function

### Synopsis

```lua
state:get( key [, default] )
```

### Arguments

`string key`
: A valid state key.

`any default`
: Optional value returned when the key is absent.

### Returns

`string, boolean, number, or any value`
: The stored value, the supplied default, or `nil`.

## Notes

Reads observe writes made earlier in the current callback transaction.

## See Also

- [`mux`](../)
- [`State`](../type-state/)
- [`State:has`](../state-has/)
- [`State:get_many`](../state-get-many/)
