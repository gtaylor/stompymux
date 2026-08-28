---
title: btech.map
linkTitle: btech.map
type: docs
weight: -10
no_list: true
sidebar_root_for: self
---

`btech.map` provides map loading, terrain and geometry queries, line of sight,
unit placement, and map-scoped messaging.

## Functions

| Function | Description |
| --- | --- |
| [`blast_zones`](blast-zones/) | Lists configured blast zones. |
| [`elevation`](elevation/) / [`terrain`](terrain/) | Returns hex elevation or terrain. |
| [`emit`](emit/) / [`hex_emit`](hex-emit/) | Broadcasts map-scoped messages. |
| [`hex_in_blast_zone`](hex-in-blast-zone/) | Tests whether a hex lies in a blast zone. |
| [`hex_line_of_sight`](hex-line-of-sight/) / [`unit_line_of_sight`](unit-line-of-sight/) | Tests line of sight. |
| [`id_to_dbref`](id-to-dbref/) | Resolves a tactical ID. |
| [`load`](load/) | Loads a map file. |
| [`range`](range/) | Calculates map distance. |
| [`set_xy`](set-xy/) | Places a unit on a map. |
| [`units`](units/) | Lists units on a map or within a range. |
| [`update_links`](update-links/) | Recursively updates map links. |
