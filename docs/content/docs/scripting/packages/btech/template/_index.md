---
title: btech.template
linkTitle: btech.template
type: docs
weight: 15
no_list: true
sidebar_root_for: self
---

`btech.template` provides immutable unit-template queries and displays.

Except for [`exists`](exists/), template queries raise
`btech.template.not_found` when the reference is missing and
`btech.template.invalid` when the referenced template is malformed.

## Functions

| Function | Description |
| --- | --- |
| [`armor`](armor/) | Returns a template's armor and internal values. |
| [`base_cost`](base-cost/) | Calculates a template's FASA base cost. |
| [`battle_value`](battle-value/) | Calculates a template's battle value. |
| [`critical_slots`](critical-slots/) | Lists a template section's critical slots. |
| [`engine`](engine/) | Returns a template's engine configuration. |
| [`exists`](exists/) | Tests whether a unit template exists. |
| [`installed_parts`](installed-parts/) | Lists parts represented by non-destroyed critical slots. |
| [`payload`](payload/) | Lists a template's weapons and ammunition. |
| [`show_critical_status`](show-critical-status/) | Sends a template's critical-status report to a player. |
| [`show_status`](show-status/) | Sends a template's status report to a player. |
| [`show_weapon_specs`](show-weapon-specs/) | Sends a template's weapon-specification report to a player. |
| [`technologies`](technologies/) | Lists a template's configured and inferred technologies. |
| [`weapons`](weapons/) | Lists the weapons mounted on a unit template. |
