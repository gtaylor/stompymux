---
title: "btech.unit.fortified"
type: docs
linkTitle: "fortified"
manualLinkTitle: "fortified"
---

Inspect or set scenario fortification. Trusted scripts own authorization.
Enabling requires settled motion, no tow relationship, no building-entry request,
and a landed unit. Disabling does not restart any action.

## Signature

```lua
btech.unit.fortified(dbref, enabled)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `enabled` | `boolean?` | Omit to inspect. |

## Returns

- `boolean Current fortification state.`
