---
title: btech package
linkTitle: btech
type: docs
weight: 20
---

# `btech`

`require("btech")` returns the built-in BattleTech API. It is available to
running Lua callbacks (not `@lua/check`) and replaces the former MUX expression
functions. Arguments use Lua strings, numbers, booleans, and dbrefs; results
are returned as Lua numbers, booleans, strings, or arrays. Mutation-only
operations return `true`. Invalid objects and arguments raise Lua errors.

The package exports these operations:

- Unit data: `armor_status`, `armor_status_ref`, `crit_slot`,
  `crit_slot_ref`, `section_status`, `crit_status`, `crit_status_ref`,
  `damage_mech`, `damages`, `engine_rating`, `engine_rating_ref`,
  `real_max_speed`, `get_weight`, `threshold`, `weapon_status`,
  `weapon_status_ref`, `weapon_stat`, `tic_weapons`, `payload_ref`,
  `show_crit_status_ref`, `show_status_ref`, and `show_weapon_specs_ref`.
- Unit mutation and loading: `load_mech`, `set_armor_status`, `set_max_speed`,
  `set_tons`, `set_xy`, `update_links`, `xcode_value`, `xcode_value_ref`, and
  `set_xcode_value`.
- Maps and geometry: `range`, `hex_emit`, `hex_in_blast_zone`,
  `hex_line_of_sight`, `mech_line_of_sight`, `map_elevation`, `map_emit`,
  `map_terrain`, `map_units`, `load_map`, `blast_zones`, and `zone_mechs`.
- Parts, stores, and economy: `add_stores`, `remove_stores`, `stores`,
  `stores_short`, `part_type`, `part_match`, `part_name`, `part_categories`,
  `parts`, `part_weight`, `part_cost`, `set_part_cost`, `unit_parts`,
  `unit_parts_ref`, `unit_fixable`, `fasa_base_cost_ref`, `battle_value`,
  `battle_value_ref`, `battle_value2_ref`, `defensive_battle_value_ref`, and
  `offensive_battle_value_ref`.
- Characters and repair: `char_list`, `char_value`, `set_char_value`,
  `make_pilot_roll`, `repair_job_count`, `tech_list`, `tech_list_ref`,
  `tech_status`, `tech_time`, and `under_repair`.
- Utilities: `design_exists`, `id_to_dbref`, `lag`, and `mech_frequencies`.
