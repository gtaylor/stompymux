---
title: mux.error.is
type: docs
toc_hide: false
---

Tests whether an error-like value has a matching code.

## Function

### Synopsis

```lua
mux.error.is(value, code)
```

### Arguments

`any value`
: A value to inspect. It matches only when it is a table with a string `code`
  field.

`string or code node code`
: A dotted code string or a node from [`mux.error.codes`](../codes/) or
  [`mux.error.namespace`](../namespace/).

### Returns

`boolean matches`
: `true` when `value.code` equals `code` or begins with `code` followed by a
  dotted segment; otherwise `false`.

## Examples

```lua
if mux.error.is(err, mux.error.codes.state) then
  -- Matches mux.state.invalid, mux.state.unavailable, and related leaves.
end
```

## Notes

[`Error:is`](../type-error/) is the equivalent method on an `Error`. Prefix
matching stops at segment boundaries, so `mux.state` does not match
`mux.statement.invalid`.

## See Also

- [`mux.error`](../)
- [`Error`](../type-error/)
- [`mux.error.codes`](../codes/)
