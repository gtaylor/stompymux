---
draft: true
title: load
type: docs
toc_hide: false
---

Loads a map file into a map object and clears its units and map objects.

## Function

### Synopsis

```lua
btech.map.load( map, name, [clear] )
```

### Arguments

`number map`
: The map-object dbref.

`string name`
: The map file name.

`boolean clear`
: Optional compatibility argument; currently ignored. It may be omitted, but
  explicitly passing `nil` raises an argument error.

### Returns

`1 success`
: Numeric `1` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets,
invalid arguments, and legacy error results raise a Lua error. The optional
`clear` argument is retained for compatibility; the current handler always
clears units and map objects. Unlike most mutating BTech calls, `load` uses the
numeric adapter and therefore returns the literal number `1`, not `true`.

## See Also

- [`btech`](../../)
