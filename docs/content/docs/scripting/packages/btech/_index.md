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

The package follows Mux conventions: required arguments are validated, some
functions ignore surplus positional arguments while others require exact arity,
options tables reject unknown fields, and mutation-only functions return no Lua
values. Failures raise structured errors. Section names and generated
abbreviations are matched case-insensitively.

## Subpackages

| Package | Description |
| --- | --- |
| [`btech.error`](error/) | Checked BattleTech error-code symbols. |
| [`btech.character`](character/) | Character values, skills, and experience. |
| [`btech.map`](map/) | Maps, geometry, line of sight, placement, and messaging. |
| [`btech.parts`](parts/) | Part catalogue and stores. |
| [`btech.player`](player/) | Player configuration. |
| [`btech.repair`](repair/) | Repair state and technician availability. |
| [`btech.system`](system/) | Server-wide BattleTech queries. |
| [`btech.template`](template/) | Immutable unit-template queries and displays. |
| [`btech.unit`](unit/) | Live-unit state, combat queries, and mutations. |

Record fields and editor types are defined in the
[generated LuaLS definitions](https://github.com/gtaylor/stompymux/blob/main/game/lua/types/btech.d.lua).
