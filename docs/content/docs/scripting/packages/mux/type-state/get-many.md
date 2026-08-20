---
title: get_many
type: docs
toc_hide: false
---

Gets every present value from a requested set of keys.

## Function

### Synopsis

```lua
state:get_many( keys )
```

### Arguments

`table keys`
: An array of valid state-key strings.

### Returns

`table values`
: A key-to-value table containing only keys that are present.

## Notes

Duplicate requested keys do not change the result. Reads observe earlier writes from the current callback transaction.

## See Also

- [`mux`](../../)
- [`State`](../)
- [`State:get`](../get/)
- [`State:entries`](../entries/)
