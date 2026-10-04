---
title: "btech.player.contact_preferences"
type: docs
linkTitle: "contact_preferences"
manualLinkTitle: "contact_preferences"
---

Read or replace saved contact-list inclusion policy for a live player.
Trusted callback code owns edit authorization; omitted replacement fields use defaults.

## Signature

```lua
btech.player.contact_preferences(player, preferences)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `player` | `integer` |  |
| `preferences` | `ContactPreferences?` |  |

## Returns

- `ContactPreferences`
