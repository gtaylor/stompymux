---
title: "btech.map.update_links_as"
type: docs
---

Rebuild reachable map routes with cycle/depth protection and atomic publication.
Rust extension retained under its descriptive name; the canonical rebuild follows the C contract.

## Signature

```lua
btech.map.update_links_as(actor, map)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `map` | `integer` |  |

## Returns

- `{buildings:integer,leaves:integer,entrances:integer,skipped:integer}`
