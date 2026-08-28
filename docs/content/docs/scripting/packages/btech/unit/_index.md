---
title: btech.unit
linkTitle: btech.unit
type: docs
weight: -20
no_list: true
sidebar_root_for: self
---

`btech.unit` exposes live-unit and template data, mutations, battle values,
and status reports.

## Functions

| Function | Description |
| --- | --- |
| [`armor_status`](armor-status/) / [`armor_status_ref`](armor-status-ref/) | Returns serialized armor values for a section. |
| [`battle_value`](battle-value/) / [`battle_value_ref`](battle-value-ref/) | Calculates a unit's battle value. |
| [`battle_value2_ref`](battle-value2-ref/) | Calculates second-generation template battle value. |
| [`crit_slot`](crit-slot/) / [`crit_slot_ref`](crit-slot-ref/) | Describes one critical slot. |
| [`crit_status`](crit-status/) / [`crit_status_ref`](crit-status-ref/) | Returns serialized critical status. |
| [`damage`](damage/) | Applies clustered damage to a live unit. |
| [`defensive_battle_value_ref`](defensive-battle-value-ref/) / [`offensive_battle_value_ref`](offensive-battle-value-ref/) | Calculates a battle-value component. |
| [`engine_rating`](engine-rating/) / [`engine_rating_ref`](engine-rating-ref/) | Returns an engine rating. |
| [`fasa_base_cost_ref`](fasa-base-cost-ref/) | Calculates a template's FASA base cost. |
| [`frequencies`](frequencies/) | Lists configured radio channels. |
| [`load`](load/) | Loads a template into a live unit. |
| [`make_pilot_roll`](make-pilot-roll/) | Makes a piloting roll and applies a failed roll. |
| [`payload_ref`](payload-ref/) | Returns a template's weapon and ammunition payload. |
| [`real_max_speed`](real-max-speed/) | Returns effective maximum speed. |
| [`section_status`](section-status/) | Returns serialized section status. |
| [`set_armor_status`](set-armor-status/) | Sets one armor-status field. |
| [`set_max_speed`](set-max-speed/) | Sets maximum speed. |
| [`set_tons`](set-tons/) | Sets tonnage and original weight. |
| [`show_crit_status_ref`](show-crit-status-ref/), [`show_status_ref`](show-status-ref/), [`show_weapon_specs_ref`](show-weapon-specs-ref/) | Sends template reports to a player. |
| [`tic_weapons`](tic-weapons/) | Lists weapons assigned to a target-interlock circuit. |
| [`weapon_status`](weapon-status/) / [`weapon_status_ref`](weapon-status-ref/) | Returns serialized weapon status. |
