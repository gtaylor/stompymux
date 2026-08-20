---
title: keys
type: docs
toc_hide: false
---

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

- [`mux`](../../)
- [`State`](../)
- [`State:entries`](../entries/)
