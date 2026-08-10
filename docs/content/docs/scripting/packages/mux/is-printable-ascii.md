---
title: mux.is_printable_ascii
type: docs
toc_hide: false
---

Tests whether every byte in a string is printable ASCII.

## Function

### Synopsis

```lua
mux.is_printable_ascii( value )
```

### Arguments

`string value`
: The byte string to inspect.

### Returns

`boolean printable`
: Whether every byte is between space (`0x20`) and `~` (`0x7e`), inclusive.

## Examples

```lua
assert(mux.is_printable_ascii("Atlas-1"))
assert(not mux.is_printable_ascii("café"))
```

## Notes

The empty string is valid. Control bytes, embedded NUL bytes, DEL, and non-ASCII UTF-8 characters return `false`.

## See Also

- [`mux`](../)
