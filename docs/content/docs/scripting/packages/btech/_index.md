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
dbrefs rather than [`Object`](../mux/world/type-object/) handles. Failures
reported by legacy handlers become `btech.failed`, while exceeding the bridge's
argument limit raises `mux.arg.invalid`. Supplying `nil`, a table, a function,
or userdata where the bridge expects a scalar instead raises an ordinary Lua
type error. Some legacy handlers do not safely guard too few arguments and may
abort; the required LuaLS parameters describe the supported call shapes.

The LuaLS signatures document each argument's canonical semantic type. For
legacy compatibility, the native bridge also stringifies numeric scalar
arguments and converts booleans to `"1"` or `"0"` before invoking a handler;
these coercions are intentionally not repeated in every signature. The legacy
handlers match character list kinds, part categories, part-name size selectors,
weapon-stat selectors, critical-slot field selectors, xcode field names, and
full unit section names without regard to case. Part-name sizes are selected by
their first letter. For a unit section that is not a full-name match, the legacy
resolver uses a class-dependent one- or two-character prefix and may ignore
trailing characters. The documentation and LuaLS aliases continue to use
canonical spellings.

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
Lua numbers, and return a flat array. Other producer delimiters remain inside
tokens, while spaces inside a display name split that name across items; see
the affected function pages for producer-specific limitations.
