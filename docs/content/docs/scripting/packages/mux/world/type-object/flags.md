---
title: flags
type: docs
toc_hide: false
---

Creates a typed flag collection for this object.

## Synopsis

```lua
local flags = object:flags()
```

Returns a [`Flags`](../../type-flags/) handle. The object must be live, and the
method is unavailable during `@lua/check`.

## See Also

- [`mux.world.flags`](../../flags/)
- [`Object:powers`](../powers/)
