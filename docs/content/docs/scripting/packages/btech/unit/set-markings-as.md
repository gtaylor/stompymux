---
title: "btech.unit.set_markings_as"
type: docs
linkTitle: "set_markings_as"
manualLinkTitle: "set_markings_as"
---

Wizard-only literal markings, at most 16383 bytes; empty clears. Callback failures roll back.
Rust extension retained under its descriptive name; the canonical setter follows the C contract.

## Signature

```lua
btech.unit.set_markings_as(actor, unit, markings)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `unit` | `integer` |  |
| `markings` | `string` |  |

## Returns

- `boolean success`
