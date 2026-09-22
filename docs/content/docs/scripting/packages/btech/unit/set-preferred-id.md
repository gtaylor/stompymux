---
title: "btech.unit.set_preferred_id"
type: docs
linkTitle: "set_preferred_id"
manualLinkTitle: "set_preferred_id"
---

Set the saved two-letter battlefield ID preference; nil clears it without consuming dice.

## Signature

```lua
btech.unit.set_preferred_id(unit, id)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `DbRef\|Object` | Constructed unit. |
| `id` | `string\|nil` | Exactly two ASCII letters; nil clears the preference. |

## Returns

No values.
