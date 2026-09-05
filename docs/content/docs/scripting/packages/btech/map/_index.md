---
title: btech.map
linkTitle: btech.map
type: docs
weight: -10
no_list: true
sidebar_root_for: self
---

`btech.map` provides maps, geometry, line of sight, placement, and messaging.

## Functions

| Function | Description |
| --- | --- |
| [`blast_zones`](blast-zones/) | Lists the blast zones configured on a map. |
| [`cargo_transfer_point`](cargo-transfer-point/) | Returns a map's cargo-transfer point. |
| [`elevation`](elevation/) | Returns the elevation of a map hex. |
| [`emit`](emit/) | Emits a message to units on a map. |
| [`in_blast_zone`](in-blast-zone/) | Tests whether a map hex lies in a configured blast zone. |
| [`line_of_sight`](line-of-sight/) | Tests line of sight from a unit to another unit or hex. |
| [`link`](link/) | Returns a child map's parent-link configuration. |
| [`load`](load/) | Loads map data from a named map file. |
| [`place_unit`](place-unit/) | Places a live unit at a position on a map. |
| [`range`](range/) | Calculates the distance between two units or positions on a map. |
| [`set_cargo_transfer_point`](set-cargo-transfer-point/) | Sets or clears a map's cargo-transfer point. |
| [`set_link`](set-link/) | Sets or clears a child map's parent-link configuration. |
| [`terrain`](terrain/) | Returns the terrain type of a map hex. |
| [`unit_by_id`](unit-by-id/) | Finds a unit by its tactical ID. |
| [`units`](units/) | Lists the live units on a map. |
| [`update_links`](update-links/) | Recursively updates a map and its linked child maps. |
