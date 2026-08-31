---
title: mux.world.flags
type: docs
weight: -50
toc_hide: false
---

`mux.world.flags` is an immutable namespace of typed `Flag` constants. Use
these values with [`Flags`](../type-flags/) methods; raw strings are rejected.

| Constant | Constant | Constant | Constant |
| --- | --- | --- | --- |
| `ANSI` | `AUDIBLE` | `AUDITORIUM` | `BLIND` |
| `CONNECTED` | `DARK` | `FLOATING` | `GAGGED` |
| `GOING` | `HALTED` | `IN_CHARACTER` | `LIGHT` |
| `MONITOR` | `NO_COMMAND` | `SAFE` | `SUSPECT` |
| `TRANSPARENT` | `WIZARD` | `XCODE` | `ZOMBIE` |

Constants compare by flag identity and stringify to the displayed uppercase
name. Unknown lookups and attempts to modify the namespace raise
`mux.flag.invalid`. Command-only aliases from `[aliases.flags]` are not
constants.

```lua
local flags = mux.world.object(ctx.object):flags()
flags:add(mux.world.flags.SAFE)
```
