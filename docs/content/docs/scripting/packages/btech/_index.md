---
title: "btech package"
linkTitle: "btech"
type: docs
weight: 20
sidebar_root_for: self
no_list: true
---

`require("btech")` returns the native, typed BattleTech API. Gameplay calls are unavailable during `@lua/check`.

## Subpackages

| Package | Description |
| --- | --- |
| [`btech.autopilot`](autopilot/) | Typed autopilot constants; queue and control calls are not implemented. |
| [`btech.cargo`](cargo/) | Cockpit stock reports and cargo transfers. |
| [`btech.character`](character/) | Character values, skills, and experience. |
| [`btech.database`](database/) | Explicit BattleTech world checkpoints. |
| [`btech.error`](error/) | Checked BattleTech error-code symbols. |
| [`btech.gunner`](gunner/) | Gunner station registration, ownership, and inspection. |
| [`btech.inventory`](inventory/) | Loose-parts inventory and stock changes. |
| [`btech.map`](map/) | Maps, geometry, line of sight, placement, and messaging. |
| [`btech.parts`](parts/) | Part catalogue and stores. |
| [`btech.player`](player/) | Saved player configuration and preferences. |
| [`btech.repair`](repair/) | Immediate repairs and technician scheduling. |
| [`btech.runtime`](runtime/) | Wizard runtime diagnostics. |
| [`btech.system`](system/) | Server-wide BattleTech queries. |
| [`btech.template`](template/) | Unit-template inspection and displays. |
| [`btech.unit`](unit/) | Live-unit state, combat queries, and mutations. |
| [`btech.weapon`](weapon/) | Runtime weapon settings. |


## Constants

- [`btech.errors`](errors/)

Functions require a live game callback unless their page states otherwise. `DbRef` is an integer object reference; `Object` is a live world handle.

See the [value types](types/) used in signatures and returned records.
