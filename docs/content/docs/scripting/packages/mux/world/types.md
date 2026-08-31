---
title: mux.world.types
type: docs
weight: -10
toc_hide: false
---

Provides immutable typed constants for native database object kinds.

## Constants

| Constant | Description |
| --- | --- |
| `mux.world.types.ROOM` | A room. |
| `mux.world.types.THING` | A thing. |
| `mux.world.types.EXIT` | An exit. |
| `mux.world.types.PLAYER` | A player. |

Constants compare by native type identity within one Lua runtime. Their string
forms are their canonical uppercase names. Unknown lookups and attempted
mutation raise `mux.arg.invalid`.

Use these constants with [`create_object`](../create-object/),
[`list_objects`](../list-objects/), [`Object:type`](../type-object/type/), and
[`Object:contents`](../type-object/contents/) rather than comparing or passing
magic strings. `PLAYER` identifies existing players but is not accepted by
`create_object`.

## See Also

- [`mux`](../../)
- [`mux.world`](../)
- [`Object`](../type-object/)
