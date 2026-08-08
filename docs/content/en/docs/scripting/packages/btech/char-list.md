---
title: btech.char_list
linkTitle: btech.char_list
type: docs
weight: 203
---

# `btech.char_list`

Lists character value names in a requested category.

## Function

### Synopsis

```lua
btech.char_list( kind, [character] )
```

### Arguments

`string kind`
: `"skills"`, `"advantages"`, or `"attributes"`.

`number or string character`
: Optional character dbref or player name used to filter learned values.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
