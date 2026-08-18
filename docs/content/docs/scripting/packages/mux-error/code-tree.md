---
title: mux.error.code_tree
type: docs
toc_hide: false
---

Returns the checked native code tree for a namespace root.

## Function

### Synopsis

```lua
local btech_codes = mux.error.code_tree("btech")
```

### Arguments

`string root`
: A native namespace root: `mux`, `btech`, or `testing`.

### Returns

`table codes`
: The cached checked symbol tree for the root. Repeated calls for the same root
  return the same table object.

Unknown roots raise `mux.arg.invalid`.

## See Also

- [`mux.error`](../)
- [`mux.error.codes`](../codes/)
- [`btech.error.codes`](../../btech-error/codes/)
