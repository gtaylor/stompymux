---
title: "btech.unit.set_preferred_id_as"
type: docs
---

Wizard-only saved ID preference; does not change the current label or consume dice.
Rust extension retained under its descriptive name; the canonical setter follows the C contract.

## Signature

```lua
btech.unit.set_preferred_id_as(actor, unit, value)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` | Wizard actor. |
| `unit` | `integer` | Constructed unit; no placement or power requirement. |
| `value?` | `string` | Exactly two ASCII letters; nil or empty clears the preference. |

## Returns

- `string? preferred_id Normalized uppercase preference, or nil when cleared.`
