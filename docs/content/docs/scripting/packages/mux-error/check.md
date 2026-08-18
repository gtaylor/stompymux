---
title: mux.error.check
type: docs
toc_hide: false
---

Returns a successful value or raises the supplied failure.

## Function

### Synopsis

```lua
mux.error.check(value, err)
```

### Arguments

`any value`
: The result to test. Any Lua truthy value is returned unchanged.

`any err`
: The Lua value to raise when `value` is `false` or `nil`; normally an
  [`Error`](../type-error/).

### Returns

`any value`
: The original truthy value.

## Examples

```lua
local record = mux.error.check(load_record(id), err)
```

## Notes

Lua considers only `false` and `nil` falsey. `check` does not normalize the
raised value, so use [`mux.error.new`](../new/) when a structured error is
needed.

## See Also

- [`mux.error`](../)
- [`mux.error.new`](../new/)
- [`mux.error.pcall`](../pcall/)
