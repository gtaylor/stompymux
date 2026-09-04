---
title: frequencies
type: docs
toc_hide: false
---

Lists the configured radio channels of a live unit.

## Function

### Synopsis

```lua
btech.unit.frequencies( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets,
invalid arguments, and legacy error results raise a Lua error. The flattened
array reflects the legacy channel serialization rather than structured channel
records. The producer separates records with commas, but the shared list
adapter splits only on spaces and `|`; a comma can therefore merge the end of
one channel record with the start of the next token.

## See Also

- [`btech`](../../)
