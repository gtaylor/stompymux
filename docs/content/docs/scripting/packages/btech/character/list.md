---
draft: true
title: list
type: docs
toc_hide: false
---

Lists character value names in a requested category.

## Function

### Synopsis

```lua
btech.character.list( kind, [character] )
```

### Arguments

`string kind`
: `"skills"`, `"advantages"`, or `"attributes"`, matched without regard to
  case.

`number or string character`
: Optional character dbref or player name used to filter learned values. It may
  be omitted, but explicitly passing `nil` raises an argument error.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
