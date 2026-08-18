---
title: mux.error.new
type: docs
toc_hide: false
---

Creates a structured error value without raising it.

## Function

### Synopsis

```lua
mux.error.new({ code = "area.missing", message = "area was not found", detail = {} })
```

### Arguments

`table fields`
: An error definition with the required `code` and `message` fields, and
  optional `detail` and `cause` fields.

`string or code node fields.code`
: A dotted code string or a node from [`mux.error.codes`](../codes/) or
  [`mux.error.namespace`](../namespace/).

`string fields.message`
: A readable description of the error.

`any fields.detail`
: Optional structured context for the failure.

`any fields.cause`
: Optional earlier error or other Lua value.

### Returns

`Error error`
: The new structured error value.

## Examples

```lua
return nil, mux.error.new({
  code = mux.error.codes.state.unavailable,
  message = "state cannot be read outside a callback",
})
```

## Notes

This function copies `code`, `message`, and non-`nil` `detail` and `cause`
into a new `Error`; it does not validate or normalize the cause.

## See Also

- [`mux.error`](../)
- [`Error`](../type-error/)
- [`mux.error.raise`](../raise/)
