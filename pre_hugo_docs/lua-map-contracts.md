# BattleTech map Lua contract evidence

The authoritative comparison is `btmux-khi/src/mux/lua/packages/btech/map/btech_map_bindings.c` and `btech_map_los_bindings.c`. The rows below are source comparisons; the C and Rust runtimes were not paired in one executable test. Rust regressions are in `tests/lua_btech_map_contracts.rs`.

All object parameters accept a numeric dbref or generation-checked `Object`. Registered maps and units reject missing, garbage, `GOING`, wrong-special, and stale values through the common structured-error helpers. Mutations return zero Lua values and run inside the existing Lua transaction, so an uncaught failure restores world and effects.

| Symbol | C-derived input/default contract | Return, failure, and effect contract | Rust regression |
|---|---|---|---|
| `elevation` | map, strict `{x,y}`; extra positional args ignored | tile elevation integer; invalid/out-of-map record is `mux.arg.invalid` argument 2 | `terrain_zones_cargo_links_and_strict_errors_match_c_shapes` |
| `terrain` | map, strict `{x,y}` | canonical snake-case terrain string; unknown/out-of-map fails | same |
| `unit_by_id` | unit-or-map origin, exactly two ASCII bytes | `Object` or nil; non-string and invalid length have distinct argument-2 diagnostics; wrong origin is `mux.object.invalid` argument 1 | `mixed_membership_range_lookup_los_and_exact_placement_are_canonical` |
| `units` | map; optional strict `{origin={x,y},range=nonnegative}` | mixed Mech/vehicle `Object` list in persisted slot order; skips `GOING`; corrupt refs fail | mixed-membership test |
| `blast_zones` | map | ordered `{x,y,radius}` records, including negative radii | metadata/link test |
| `in_blast_zone` | map, strict `{x,y}` | boolean by real hex-center distance; negative zones inactive | metadata/link test |
| `range` | map, from, to; endpoint is unit or strict `{x,y,z?}`; table z defaults to base elevation | C-order float32 3D map distance using `SCALEMAP`/`ZSCALE`; unit must be live and on supplied map | both tests cover table, unit, and explicit z |
| `place_unit` | unit, map, strict `{x,y,z?}`; z integral 0..10000 | zero values; exact candidate placement, common membership, altitude, and tow synchronization; `detail.reason=map_placement_failed` | mixed-membership test |
| `load` | map, nonempty string; path components forbidden | zero values; validate/load asset, then clear units and map objects; failures use `map_file_invalid`; clear effects remain transactional | alias retained and canonical error/effect coverage is grouped with existing map-load regressions |
| `update_links` | map | zero values; bounded recursive rebuild without an actor acknowledgement; candidate validation before commit | `terrain_zones_cargo_links_and_strict_errors_match_c_shapes` asserts zero returns and no actor output; authored extension persistence remains covered under `update_links_as` |
| `emit` | map, nonempty message; omitted options means all; strict options choose all/range/line_of_sight | zero values; running-recipient semantics; 2D/3D range; LOS sensor eligibility; per-recipient `$h`/`$H`; no actor acknowledgement | mixed-membership test covers all/range/LOS audiences, exclusion, substitution, and no acknowledgement |
| `cargo_transfer_point` | map | `{x,y,reveal_hint}` or nil | metadata/link test |
| `set_cargo_transfer_point` | map, strict point or explicit nil | zero values; coordinate validation; store/clear reason metadata | metadata/link test |
| `link` | child map | nil or `{parent=Object,x,y,entrances}`; `GOING` parent becomes nil and corrupt parent fails | metadata/link test |
| `set_link` | child, strict link or explicit nil; exact entrances checked against child and parent point against parent | zero values; atomic store/clear with `map_link_store_failed` reason | metadata/link test |
| `line_of_sight` | observer unit, target unit or strict `{x,y}` | `clear`, `blocked`, or `none`; different-map/invisible unit is none; hex path distinguishes clear/blocked | mixed-membership test covers clear, blocked, and invisible none |

Earlier Rust actor-taking or authored-shape extensions remain available as `load_as(actor,map,name)`, `emit_as(actor,map,message)`, `update_links_as(actor,map)`, `authored_link(child)`, and `set_authored_link(child,link)`. The canonical C names do not accept those extension tuples.

`going_handles_and_checking_mode_preserve_object_and_mutation_boundaries` covers GOING handles, finite/fractional numeric coercion boundaries, structured argument metadata, and complete public-call denial in Checking mode.
