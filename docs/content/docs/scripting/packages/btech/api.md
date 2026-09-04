---
title: API reference
type: docs
weight: 10
---

All object-valued parameters below use `DbRef|Object` unless specifically
documented as character-only. Optional object relationships return `nil` and
collections return dense `Object[]` arrays.

## Character

`btech.character` exports `catalog`, `value`, `experience_threshold`,
`set_value`, `set_skill_target`, `set_skill_experience`, and
`add_skill_experience`. Character references are `Object`-only. Setters return
no values.

## Maps

`btech.map` exports `blast_zones`, `cargo_transfer_point`,
`set_cargo_transfer_point`, `elevation`, `terrain`, `emit`, `in_blast_zone`,
`line_of_sight`, `unit_by_id`, `link`, `set_link`, `load`, `range`,
`place_unit`, `units`, and `update_links`.

Coordinates are zero-based and bounded by map width and height. Tactical IDs
are exactly two ASCII characters. `line_of_sight` returns `"none"`,
`"blocked"`, or `"clear"` for unit targets and `"blocked"` or `"clear"` for
hex targets. `load` clears attached units and map objects.

`emit` accepts no options (all), or an exact options table with audience
`"all"`, `"range"`, or `"line_of_sight"`. Range emission requires `origin`
and `range`; line-of-sight emission requires an x/y origin and forbids z/range.

## Parts and stores

`btech.parts` exports `categories`, `list`, `search`, `resolve`, `stores`,
`store_quantity`, `adjust_stores`, and `set_cost`. A `BtechPartRef` is a part
record, packed integer ID, or a unique case-insensitive exact name. Store
adjustments require a nonzero signed delta and validate before committing.

## Player configuration

`btech.player` exports `loadout`, `set_loadout`, `mechwarrior_template`,
`set_mechwarrior_template`, `ui_preferences`, and `set_ui_preferences`.
Loadout equipment uses `BtechPart` records and rejects non-personal weapons or
ammunition on weapons that do not consume it.

## Repair and system

`btech.repair` exports `needs`, `is_under_repair`, `is_fixable`, and
`technician_available_in`. `btech.system` exports `event_lag` and
`units_in_zone`.

## Live units

`btech.unit` exports the parallel status queries `armor`, `battle_value`,
`critical_slots`, `engine`, `weapons`, `installed_parts`, `payload`, and
`technologies`. It additionally exports `preferred_id`, `markings`,
`assigned_pilot`, `display_name` and their setters, plus `set_armor`,
`apply_damage`, `radio_channels`, `load_template`, `piloting_check`,
`effective_max_speed`, `section_condition`, `set_max_speed`, `set_tonnage`, and
`tic_weapons`.

Battle value is calculated from the unit's current armor, internals, movement,
tonnage, weapons, heat efficiency, and operational equipment. The returned
record contains `total`, `offensive`, and `defensive` numeric fields.

## Errors

BTech-specific errors are `btech.part.not_found`, `btech.part.ambiguous`,
`btech.part.wrong_kind`, `btech.template.not_found`,
`btech.template.invalid`, and `btech.operation.failed`. The last includes a
stable `detail.reason`. Object, argument, checking, and internal failures use
the corresponding `mux.*` error codes.
