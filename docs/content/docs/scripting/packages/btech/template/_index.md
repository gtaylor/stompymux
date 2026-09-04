---
title: btech.template
linkTitle: template
type: docs
weight: 70
---

`btech.template` is the immutable counterpart to `btech.unit`. It exports
`exists`, `armor`, `battle_value`, `critical_slots`, `engine`, `weapons`,
`installed_parts`, `payload`, `technologies`, `base_cost`, `show_status`,
`show_weapon_specs`, and `show_critical_status`.

Template references are strings. `exists` returns `false` for a missing
template and raises `btech.template.invalid` for a malformed one. Other queries
raise `btech.template.not_found` or `btech.template.invalid`. Display functions
accept a `DbRef|Object` recipient and return no Lua values.

Battle value is returned as `total`, `offensive`, and `defensive` numeric
components. Base cost is returned as a safe Lua integer.
