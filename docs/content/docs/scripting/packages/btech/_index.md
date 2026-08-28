---
title: btech package
linkTitle: btech
type: docs
weight: 20
sidebar_root_for: self
no_list: true
---

`require("btech")` returns the built-in BattleTech API. It is available to
running Lua callbacks, but its gameplay functions are unavailable during
`@lua/check`. The package adapts the server's trusted BattleTech scripting
handlers to Lua and invokes them with server authority.

Arguments use Lua strings, numbers, and booleans; object arguments are native
dbrefs rather than [`Object`](../mux/world/type-object/) handles. Invalid
objects, arguments, or legacy error results raise structured Lua errors.
Generic failures use `mux.error.codes`; BattleTech-subsystem failures use
`btech.error.codes`.

## Subpackages

| Package | Description |
| --- | --- |
| [`btech.character`](character/) | Character values, skills, experience, and piloting rolls. |
| [`btech.error`](error/) | Checked BattleTech error-code symbols. |
| [`btech.map`](map/) | Battle maps, geometry, line of sight, and map messaging. |
| [`btech.parts`](parts/) | Part catalogues, installed parts, stores, and costs. |
| [`btech.repair`](repair/) | Damage and technician-status queries. |
| [`btech.system`](system/) | Special-object fields and server-wide BattleTech queries. |
| [`btech.unit`](unit/) | Live units, templates, combat values, and status. |

Successful results are converted to the type documented for each function.
Mutation-only operations return `true`. List operations split the legacy
result on whitespace and `|`, convert numeric tokens (including `#123`) to
Lua numbers, and return a flat array.
