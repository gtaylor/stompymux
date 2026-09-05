---
title: exists
type: docs
toc_hide: false
---

Tests whether a unit template exists.

## Function

### Synopsis

```lua
btech.template.exists( reference )
```

### Arguments

`string reference`
: The unit-template reference.

### Returns

`boolean exists`
: Whether the template exists.

## Notes

An empty reference or one containing `..`, `/`, or `\` raises
`mux.arg.invalid`. A well-formed missing reference returns `false`. When the
referenced file exists but is not a valid template, the function raises
`btech.template.invalid`.

## See Also

- [`btech`](../../)
- [`btech.template`](../)
