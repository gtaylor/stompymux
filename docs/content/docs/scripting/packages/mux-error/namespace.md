---
title: mux.error.namespace
type: docs
toc_hide: false
---

Builds a checked code-symbol tree for an author-defined error namespace.

## Function

### Synopsis

```lua
local codes = mux.error.namespace("cargo", { "full", "bay.locked" })
```

### Arguments

`string prefix`
: The first code segment, or a dotted series of lower-case segments. The
  first segment must not be `mux`, `btech`, or `testing`.

`string[] names`
: An array of leaf names relative to `prefix`. Every name uses one or more
  lower-case dotted segments; later segments may also contain digits or `_`.

### Returns

`table codes`
: A code-symbol tree whose leaf and intermediate nodes can be passed wherever
  `mux.error` accepts a code.

## Examples

```lua
local codes = mux.error.namespace("cargo", { "full", "bay.locked" })
mux.error.raise(codes.bay.locked, "the cargo bay is locked")
```

## Notes

The tree follows the same lookup and mutation behavior as
[`mux.error.codes`](../codes/). `mux`, `btech`, and `testing` are reserved
first segments; a prefix whose first segment is one of them is rejected.

## See Also

- [`mux.error`](../)
- [`mux.error.codes`](../codes/)
- [`mux.error.raise`](../raise/)
