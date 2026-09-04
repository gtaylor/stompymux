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
`mux.power.invalid`. String conversion raises the invariant-only `mux.internal`
error if a registered native power name exceeds the internal conversion
buffer.

```lua
local powers = mux.world.object(ctx.object):powers()
powers:add(mux.world.powers.IDLE)
```
