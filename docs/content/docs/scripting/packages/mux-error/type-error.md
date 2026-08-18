---
title: Error
type: docs
toc_hide: false
---

An `Error` is a Lua table that identifies a failure with a stable `code` and a
human-readable `message`. Native bindings raise these values, and scripts can
create them with [`mux.error.new`](../new/) or
[`mux.error.raise`](../raise/). Use an `Error` as the second result in the
recoverable `nil, err` convention, or raise it when the current operation
cannot continue.

## Fields

`string code`
: The dotted error code. It can be compared as a string, but checked symbols
  from [`mux.error.codes`](../codes/) catch misspellings at lookup time.

`string message`
: A readable description of the failure.

`any detail`
: Optional structured context supplied by the producer. Native argument errors
  use a table containing `argument`.

`any cause`
: Optional earlier error or other Lua value. [`mux.error.wrap`](../wrap/)
  stores the normalized wrapped value here.

`string traceback`
: Optional traceback attached by [`mux.error.pcall`](../pcall/) when it catches
  a failure.

## Methods

| Method | Description |
| --- | --- |
| [`Error:is`](#is) | Tests this error's code against a leaf or namespace code. |
| [`Error:root`](#root) | Returns the deepest table reached through `cause`. |

### `is`

```lua
error:is(code)
```

`string or code node code`
: A dotted code string or a node from [`mux.error.codes`](../codes/) or
  [`mux.error.namespace`](../namespace/).

Returns `true` when this error's code is an exact match or begins with `code`
at a dotted segment boundary.

### `root`

```lua
error:root()
```

Returns this error's deepest table-valued `cause`; it returns the error itself
when there is no table-valued cause. Traversal is limited to 64 cause links.

## Notes

`tostring(error)` renders `code: message`. Code matching is exact for a leaf,
or matches a dotted prefix at a segment boundary: `mux.state` matches every
`mux.state.*` error but not `mux.statement`.

Failed callbacks are always written to the Lua problem log. The
`lua.error_reporting` setting controls player-visible notifications: `off`
sends none, `wizards` sends details only to Wizards, and `all` sends details
to everyone; otherwise notified players see `A script error occurred.`

## See Also

- [`mux.error`](../)
- [`mux.error.codes`](../codes/)
- [Lua errors](../../../errors/)
