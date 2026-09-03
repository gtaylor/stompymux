---
title: set_description
type: docs
toc_hide: false
---

Sets or clears this object's styled description.

## Function

```lua
object:set_description(description)
object:set_description(nil)
```

## Arguments

`string or nil description`
: Valid styled-text markup. `nil` or an empty string clears the description.

## Errors

- `mux.unavailable.checking` during `@lua/check`.
- `mux.arg.invalid` for invalid text or markup.
- `mux.object.invalid` if the handle is stale.
- `mux.object.unavailable` if the object is being destroyed.

## See Also

- [`Object`](../)
- [`Object:description`](../description/)
