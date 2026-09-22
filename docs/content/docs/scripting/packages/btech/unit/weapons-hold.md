---
title: "btech.unit.weapons_hold"
type: docs
linkTitle: "weapons_hold"
manualLinkTitle: "weapons_hold"
---

Read or change operator weapons hold inside a trusted callback transaction.
Hold blocks fire and TIC admission before argument decoding or loss of cover.
The setting persists through shutdown and restart; aborted callbacks restore it.

## Signature

```lua
btech.unit.weapons_hold(dbref, enabled)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Constructed unit dbref. |
| `enabled` | `boolean\|nil` | Omit to inspect without changing the setting. |

## Returns

- `boolean enabled`
