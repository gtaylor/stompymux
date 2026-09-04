---
title: pcall
type: docs
toc_hide: false
---

Calls a function and returns either its results or a normalized error with a
traceback.

## Function

### Synopsis

```lua
ok, result_or_error = mux.error.pcall(fn, ...)
```

### Arguments

`function fn`
: The function to call.

`...`
: Arguments passed to `fn`.

### Returns

`true, ...`
: On success, `true` followed by every value returned by `fn`.

`false, Error or caught-error table error`
: On failure, `false` and the raised error. A raised table with a string
  `code` field is retained; another Lua value is converted to a `mux.runtime`
  `Error`. Every returned failure has string `code` and `traceback` fields.

## Examples

```lua
local ok, value_or_err = mux.error.pcall(read_manifest, id)
if not ok then
  return nil, value_or_err
end
```

## Notes

Unlike Lua's `pcall`, this helper makes unstructured failures inspectable by
code. A retained table with a string `code` field is not copied or given the
`Error` metatable. Such a caught-error table is guaranteed only `code` and
`traceback`; it can omit `message`, `detail`, `cause`, and every `Error` method,
and it can retain arbitrary caller-defined fields. The traceback is reported as
`Lua traceback unavailable` only when the runtime cannot obtain one.

## See Also

- [`mux.error`](../)
- [`Error`](../type-error/)
- [`mux.error.wrap`](../wrap/)
