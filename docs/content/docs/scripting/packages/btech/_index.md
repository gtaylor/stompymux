---
title: btech package
linkTitle: btech
type: docs
weight: 20
sidebar_root_for: self
no_list: true
---

`require("btech")` returns the built-in BattleTech API. It is available to
running Lua callbacks, but not during `@lua/check`. The package adapts the
server's trusted BattleTech scripting handlers to Lua and invokes them with
server authority.

Arguments use Lua strings, numbers, and booleans; object arguments are native
dbrefs rather than [`Object`](../mux/type-object/) handles. Invalid objects,
arguments, or legacy error results raise structured Lua errors. Generic
failures use `mux.error.codes`; BattleTech-subsystem failures use
`btech.error.codes`. Successful results are converted to the type documented
for each function. Mutation-only operations return `true`. List operations
split the legacy result on whitespace and `|`, convert numeric tokens (including
`#123`) to Lua numbers, and return a flat array.

## Functions

### Unit data

| Function | Description |
| --- | --- |
| [`btech.armor_status`](armor-status/) | Returns serialized armor values for one section of a live unit. |
| [`btech.armor_status_ref`](armor-status-ref/) | Returns serialized armor values for one section of a unit template. |
| [`btech.crit_slot`](crit-slot/) | Describes one critical slot on a live unit. |
| [`btech.crit_slot_ref`](crit-slot-ref/) | Describes one critical slot in a unit template. |
| [`btech.section_status`](section-status/) | Returns serialized status for one section of a live unit. |
| [`btech.crit_status`](crit-status/) | Returns serialized critical-slot status for one section of a live unit. |
| [`btech.crit_status_ref`](crit-status-ref/) | Returns serialized critical-slot status for one section of a unit template. |
| [`btech.damage_mech`](damage-mech/) | Applies clustered damage to a live unit. |
| [`btech.damages`](damages/) | Returns the formatted repair-job description for a live unit. |
| [`btech.engine_rating`](engine-rating/) | Returns the engine rating of a live unit. |
| [`btech.engine_rating_ref`](engine-rating-ref/) | Returns the engine rating of a unit template. |
| [`btech.real_max_speed`](real-max-speed/) | Returns a live unit's effective maximum speed. |
| [`btech.get_weight`](get-weight/) | Returns a part's weight in tons. |
| [`btech.payload_ref`](payload-ref/) | Returns the weapon and ammunition payload of a unit template. |
| [`btech.show_crit_status_ref`](show-crit-status-ref/) | Sends a template's critical-status display to a player. |
| [`btech.show_status_ref`](show-status-ref/) | Sends a unit template's status display to a player. |
| [`btech.show_weapon_specs_ref`](show-weapon-specs-ref/) | Sends a unit template's weapon-specification display to a player. |
| [`btech.threshold`](threshold/) | Returns the configured experience threshold for a skill. |
| [`btech.tic_weapons`](tic-weapons/) | Lists the weapons assigned to a unit's target-interlock circuit. |
| [`btech.weapon_status`](weapon-status/) | Returns serialized weapon status for a live unit or one section. |
| [`btech.weapon_status_ref`](weapon-status-ref/) | Returns serialized weapon status for a unit template or one section. |
| [`btech.weapon_stat`](weapon-stat/) | Returns one numeric weapon-catalog statistic as text. |

### Unit mutation and loading

| Function | Description |
| --- | --- |
| [`btech.xcode_value`](xcode-value/) | Reads a script-visible native field from a live special object. |
| [`btech.xcode_value_ref`](xcode-value-ref/) | Reads a script-visible native field from a unit template. |
| [`btech.load_map`](load-map/) | Loads a map file into a map object and clears its units and map objects. |
| [`btech.load_mech`](load-mech/) | Loads a unit template into a live unit object. |
| [`btech.set_armor_status`](set-armor-status/) | Sets one armor-status field on a live unit section. |
| [`btech.set_max_speed`](set-max-speed/) | Sets a live unit's maximum speed and corrects its current speed. |
| [`btech.set_tons`](set-tons/) | Sets a live unit's tonnage and original weight. |
| [`btech.set_xcode_value`](set-xcode-value/) | Writes a script-writable native field on a live special object. |
| [`btech.set_xy`](set-xy/) | Places a live unit on a map at specified coordinates. |
| [`btech.update_links`](update-links/) | Recursively updates links associated with a map. |

### Maps and geometry

| Function | Description |
| --- | --- |
| [`btech.range`](range/) | Calculates distance between units or map coordinates. |
| [`btech.hex_emit`](hex-emit/) | Broadcasts a message from one map hex. |
| [`btech.hex_in_blast_zone`](hex-in-blast-zone/) | Tests whether a map hex lies in a configured blast zone. |
| [`btech.hex_line_of_sight`](hex-line-of-sight/) | Tests whether a live unit has unobstructed line of sight to a map hex. |
| [`btech.blast_zones`](blast-zones/) | Lists blast-zone coordinates and radii on a map. |
| [`btech.mech_line_of_sight`](mech-line-of-sight/) | Tests line of sight between two live units. |
| [`btech.map_elevation`](map-elevation/) | Returns the elevation of a map hex. |
| [`btech.map_emit`](map-emit/) | Broadcasts a message to all or nearby units on a map. |
| [`btech.map_terrain`](map-terrain/) | Returns the terrain code of a map hex. |
| [`btech.map_units`](map-units/) | Lists all units on a map or those within a 2D or 3D range. |

### Parts, stores, and economy

| Function | Description |
| --- | --- |
| [`btech.add_stores`](add-stores/) | Adds a quantity of a part to an object's stores. |
| [`btech.fasa_base_cost_ref`](fasa-base-cost-ref/) | Calculates the FASA base cost of a unit template. |
| [`btech.battle_value`](battle-value/) | Calculates the battle value of a live unit. |
| [`btech.battle_value_ref`](battle-value-ref/) | Calculates the battle value of a unit template. |
| [`btech.battle_value2_ref`](battle-value2-ref/) | Calculates the second-generation battle value of a unit template. |
| [`btech.defensive_battle_value_ref`](defensive-battle-value-ref/) | Calculates the defensive battle-value component of a unit template. |
| [`btech.offensive_battle_value_ref`](offensive-battle-value-ref/) | Calculates the offensive battle-value component of a unit template. |
| [`btech.part_cost`](part-cost/) | Returns the configured cost of a part. |
| [`btech.part_type`](part-type/) | Returns the broad category of a part. |
| [`btech.part_match`](part-match/) | Finds packed part IDs whose names match a string. |
| [`btech.part_name`](part-name/) | Returns a name for a packed part ID. |
| [`btech.part_categories`](part-categories/) | Lists the canonical categories accepted by `btech.parts`. |
| [`btech.parts`](parts/) | Lists canonical long part names in one category. |
| [`btech.part_weight`](part-weight/) | Returns a part's weight in tons. |
| [`btech.remove_stores`](remove-stores/) | Removes a quantity of a part from an object's stores. |
| [`btech.set_part_cost`](set-part-cost/) | Sets the configured cost of a part. |
| [`btech.stores`](stores/) | Returns a part quantity or lists an object's stored parts. |
| [`btech.stores_short`](stores-short/) | Returns a part quantity or lists stored parts using short names. |
| [`btech.unit_fixable`](unit-fixable/) | Tests whether a live unit can be repaired. |
| [`btech.unit_parts`](unit-parts/) | Lists the parts installed on a live unit. |
| [`btech.unit_parts_ref`](unit-parts-ref/) | Lists the parts installed in a unit template. |

### Characters and repair

| Function | Description |
| --- | --- |
| [`btech.char_list`](char-list/) | Lists character value names in a requested category. |
| [`btech.char_value`](char-value/) | Gets a character attribute, skill level, target, experience, or experience threshold. |
| [`btech.make_pilot_roll`](make-pilot-roll/) | Makes a piloting roll and causes a fall when it fails. |
| [`btech.repair_job_count`](repair-job-count/) | Returns the number of pending repair jobs on a live unit. |
| [`btech.set_char_value`](set-char-value/) | Sets a character value or adjusts skill experience. |
| [`btech.tech_list`](tech-list/) | Lists the parts needed to repair a live unit. |
| [`btech.tech_list_ref`](tech-list-ref/) | Lists the parts needed to repair a unit template. |
| [`btech.tech_status`](tech-status/) | Returns formatted repair status for a live unit. |
| [`btech.tech_time`](tech-time/) | Runs the legacy technician-time query. |
| [`btech.under_repair`](under-repair/) | Tests whether a live unit has an active repair event. |

### Utilities

| Function | Description |
| --- | --- |
| [`btech.design_exists`](design-exists/) | Tests whether a unit template exists. |
| [`btech.id_to_dbref`](id-to-dbref/) | Resolves a two-character tactical ID on a unit's map. |
| [`btech.lag`](lag/) | Returns the current BattleTech event lag. |
| [`btech.mech_frequencies`](mech-frequencies/) | Lists the configured radio channels of a live unit. |
| [`btech.zone_mechs`](zone-mechs/) | Lists live unit objects assigned to a zone. |

## Errors

| Function | Description |
| --- | --- |
| [`btech.error`](../btech-error/) | Checked BattleTech error-code symbols. |
