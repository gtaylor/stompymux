---
title: "btech.unit.losemit"
type: docs
linkTitle: "losemit"
manualLinkTitle: "losemit"
---

Wizard-only literal emote to running units currently seeing the source; source cockpit excluded.
Privately confirms completion. Entire publication rolls back on failure.

## Signature

```lua
btech.unit.losemit(actor, unit, message)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `unit` | `integer` | Placed physical unit; need not be running or piloted by actor. |
| `message` | `string` | Empty text and leading apostrophes retain ordinary emote semantics. |

## Returns

- `integer observers Number of addressed observer units.`
