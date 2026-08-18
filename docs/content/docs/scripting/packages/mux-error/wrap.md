---
title: mux.error.wrap
type: docs
toc_hide: false
---

Creates a structured error that preserves an earlier failure as its cause.

## Function

### Synopsis

```lua
mux.error.wrap(err, code, message)
```

### Arguments

`any err`
: The earlier failure. A table with a string `code` field is retained; another
  Lua value is normalized to a `mux.runtime` error before becoming the cause.

`string or code node code`
: A dotted code string or a node from [`mux.error.codes`](../codes/) or
  [`mux.error.namespace`](../namespace/).

`string message`
: A readable description of the additional context.

### Returns

`Error error`
: A new error with `error.cause` set to the retained or normalized `err`.

## Examples

```lua
local ok, err = mux.error.pcall(write_manifest)
if not ok then
  return nil, mux.error.wrap(err, "cargo.manifest.failed", "could not save manifest")
end
```

## Notes

Use [`Error:root`](../type-error/) to retrieve the deepest table-valued cause.
`wrap` does not attach a traceback; [`mux.error.pcall`](../pcall/) does.

## See Also

- [`mux.error`](../)
- [`Error`](../type-error/)
- [`mux.error.pcall`](../pcall/)
