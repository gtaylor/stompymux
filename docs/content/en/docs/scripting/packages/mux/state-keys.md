---
title: State:keys
linkTitle: State:keys
type: docs
weight: 227
---

# `State:keys`

Lists the keys in this state namespace.

## Function

### Synopsis

```lua
state:keys( )
```

### Arguments

None.

### Returns

`table keys`
: An array of strings sorted by key.

## Notes

Enumeration requires an active callback transaction.

## See Also

- [`mux`](../)
- [`State`](../type-state/)
- [`State:entries`](../state-entries/)
