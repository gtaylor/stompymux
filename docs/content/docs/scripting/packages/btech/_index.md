---
title: btech package
linkTitle: btech
type: docs
weight: 20
sidebar_root_for: self
no_list: true
---

`require("btech")` returns the native, typed BattleTech API. Gameplay calls are
unavailable during `@lua/check` and raise `mux.unavailable.checking` there.

Database-object parameters accept either a numeric `DbRef` or a same-runtime
[`Object`](../mux/world/type-object/) handle. Character parameters require an
`Object`. Returned database references are `Object` handles; absent optional
relationships are `nil`, and object collections are dense arrays.

The package follows Mux conventions: required arguments are validated, surplus
positional arguments are ignored, options tables reject unknown fields, and
mutation-only functions return no Lua values. Failures raise structured errors
instead of returning `#-N` strings. Sections accept an exact class-specific
name or generated abbreviation, case-insensitively.

## Subpackages

| Package | Description |
| --- | --- |
| [`btech.character`](api/#character) | Character values, skills, and experience. |
| [`btech.map`](api/#maps) | Maps, geometry, line of sight, placement, and messaging. |
| [`btech.parts`](api/#parts-and-stores) | Part catalogues and stores. |
| [`btech.player`](api/#player-configuration) | Player configuration. |
| [`btech.repair`](api/#repair-and-system) | Repair state and technician queries. |
| [`btech.system`](api/#repair-and-system) | Server-wide BattleTech queries. |
| [`btech.template`](template/) | Immutable unit-template queries and displays. |
| [`btech.unit`](api/#live-units) | Live units, combat state, and mutations. |

See the [API reference](api/) for the complete exported surface and the
generated `game/lua/types/btech.d.lua` for record fields and editor types.
