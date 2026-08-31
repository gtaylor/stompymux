---
title: mux.world.powers
type: docs
weight: -20
toc_hide: false
---

`mux.world.powers` is an immutable namespace of typed `Power` constants. Use
these values with [`Powers`](../type-powers/) methods; raw strings are rejected.

| Constant | Purpose |
| --- | --- |
| `IDLE` | Exempts a connected player from the inactivity timeout. |

Constants compare by power identity and stringify to their uppercase name.
Unknown lookups and attempts to modify the namespace raise
`mux.power.invalid`.

```lua
local powers = mux.world.object(ctx.object):powers()
powers:add(mux.world.powers.IDLE)
```
