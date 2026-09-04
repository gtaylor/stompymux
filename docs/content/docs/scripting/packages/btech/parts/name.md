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
: Canonically `"short"`, `"long"`, or `"vlong"`. The legacy handler examines
  only the first letter without regard to case, so any non-empty string
  beginning with `s`, `l`, or `v` selects the corresponding form.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets,
invalid arguments, and legacy error results raise a Lua error. The `size`
argument is required: the legacy handler does not safely reject its omission
and may abort instead of raising a checked Lua error.

## See Also

- [`btech`](../../)
