---
title: "btech.map.emit_as"
type: docs
linkTitle: "emit_as"
manualLinkTitle: "emit_as"
---

Wizard broadcast to occupants of running, conscious, unblinded units in map-slot order.
Does not require sensor contacts; all notices and the private confirmation roll back together.
Rust extension retained under its descriptive name; the canonical emit follows the C contract.

## Signature

```lua
btech.map.emit_as(actor, dbref, text)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `text` | `string` | Leading spaces are removed; empty messages are rejected. |

## Returns

- `integer[] Eligible unit dbrefs, including units with empty cockpits.`
