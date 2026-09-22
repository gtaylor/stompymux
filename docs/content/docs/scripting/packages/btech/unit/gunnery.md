---
title: "btech.unit.gunnery"
type: docs
linkTitle: "gunnery"
manualLinkTitle: "gunnery"
---

Read current connected pilot gunnery under configured weapon-family rules; default six without one.

## Signature

```lua
btech.unit.gunnery(dbref, weapon)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Unit dbref. |
| `weapon` | `integer` | Zero-based weapon index. |

## Returns

- `integer Signed skill target; no XP award or firing permission.`
