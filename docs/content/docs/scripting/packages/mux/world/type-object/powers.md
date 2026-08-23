---
title: powers
type: docs
toc_hide: false
---

Creates a typed power collection for this object.

## Synopsis

```lua
local powers = object:powers()
```

Returns a [`Powers`](../../type-powers/) handle. The object must be live, and
the method is unavailable during `@lua/check`.

## See Also

- [`mux.world.powers`](../../powers/)
- [`Object:flags`](../flags/)
