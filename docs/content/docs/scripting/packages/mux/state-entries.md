---
title: State:entries
type: docs
toc_hide: false
---

Lists the entries in this state namespace.

## Function

### Synopsis

```lua
state:entries( )
```

### Arguments

None.

### Returns

`table entries`
: An array of `{ key = ..., value = ... }` records sorted by key.

## Notes

Enumeration requires an active callback transaction and includes earlier writes from that transaction.

## See Also

- [`mux`](../)
- [`State`](../type-state/)
- [`State:keys`](../state-keys/)
- [`State:get_many`](../state-get-many/)
