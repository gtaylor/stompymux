---
draft: true
title: match
type: docs
toc_hide: false
---

Finds packed part IDs whose names match a string.

## Function

### Synopsis

```lua
btech.parts.match( query )
```

### Arguments

`string query`
: The part-name text to match.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
