---
title: name
type: docs
toc_hide: false
---

Returns a name for a packed part ID.

## Function

### Synopsis

```lua
btech.parts.name( part, size )
```

### Arguments

`number part`
: The packed part ID.

`string size`
: `"short"`, `"long"`, or `"vlong"`.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
