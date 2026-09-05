---
title: resolve
type: docs
toc_hide: false
---

Resolves a part reference to its canonical record.

## Function

### Synopsis

```lua
btech.parts.resolve( part )
```

### Arguments

`BtechPartRef part`
: A part record, packed ID, or unique name.

### Returns

`BtechPart|nil part`
: The canonical part, or `nil` when the reference is not found.

## See Also

- [`btech`](../../)
- [`btech.parts`](../)
