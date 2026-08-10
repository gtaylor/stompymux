---
title: State:set_many
type: docs
toc_hide: false
---

Applies several persistent state updates.

## Function

### Synopsis

```lua
state:set_many( values )
```

### Arguments

`table values`
: A string-keyed table of supported state values.

### Returns

Nothing.

## Notes

All updates join the current callback transaction. Every key must be a valid string and every value must be a string, boolean, or finite number. Lua tables cannot carry a `nil` entry, so use `State:set` or `State:delete` to remove keys.

## See Also

- [`mux`](../)
- [`State`](../type-state/)
- [`State:set`](../state-set/)
- [`State:delete`](../state-delete/)
