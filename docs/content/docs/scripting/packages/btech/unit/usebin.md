---
title: "btech.unit.usebin"
type: docs
linkTitle: "usebin"
manualLinkTitle: "usebin"
---

Select an ammunition section, or clear with nil or '-'. Requires a conscious assigned
pilot, map placement and an intact non-recycling ammunition weapon, but no reactor power.

## Signature

```lua
btech.unit.usebin(dbref, pilot, weapon, section)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `weapon` | `integer` | Zero-based weapon number. |
| `section` | `string\|nil` | Cockpit section name or abbreviation. |

## Returns

- `boolean`
