---
title: mux.world.access
type: docs
toc_hide: false
---

`mux.world.access` is an immutable namespace of typed `Access` constants. Use
these values for the optional `access` field in Lua command entries; raw
strings are rejected.

| Constant | Allowed invokers |
| --- | --- |
| `PUBLIC` | Everyone. This is the default when `access` is omitted. |
| `WIZARD` | Wizards and God. |
| `GOD` | God only. |

Constants compare by access identity and stringify to the displayed uppercase
name. Unknown lookups and attempts to modify a constant or the namespace raise
`mux.access.invalid`.

```lua
return {
  commands = {
    {
      pattern = "^rebuild$",
      access = mux.world.access.WIZARD,
      handler = rebuild,
    },
  },
}
```
