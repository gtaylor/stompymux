---
title: "btech.unit.contacts"
type: docs
linkTitle: "contacts"
manualLinkTitle: "contacts"
---

Read acquired contacts the unit still perceives; no acquisition rolls.

## Signature

```lua
btech.unit.contacts(dbref, preferences)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Running observer unit dbref. |
| `preferences` | `BattleContactPreferences?` | Optional inclusion filter; omitted lists all acquired contacts. |

## Returns

- `BattleContactView[]`
