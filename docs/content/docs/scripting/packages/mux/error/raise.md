---
title: raise
type: docs
toc_hide: false
---

Raises a structured error.

## Function

### Synopsis

```lua
mux.error.raise(code, message [, detail])
```

### Arguments

`string or code node code`
: A code string or a node from [`mux.error.codes`](../codes/) or
  [`mux.error.namespace`](../namespace/). Plain strings are accepted verbatim;
  this function does not validate their syntax.

`string message`
: A readable description of the failure.

`any detail`
: Optional structured context stored on the raised `Error`.

### Returns

Nothing; this function raises an `Error`.

## Examples

```lua
if cargo.full then
  mux.error.raise(codes.full, "the cargo bay is full", { capacity = 20 })
end
```

## Notes

Use `return nil, err` rather than raising when the caller is expected to
recover from the condition. Prefer a checked code node or a conventionally
dotted plain string even though plain strings are not syntax-checked.

## See Also

- [`mux.error`](../)
- [`Error`](../type-error/)
- [`mux.error.new`](../new/)
