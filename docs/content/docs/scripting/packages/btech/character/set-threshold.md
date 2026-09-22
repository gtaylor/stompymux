---
title: "btech.character.set_threshold"
type: docs
linkTitle: "set_threshold"
manualLinkTitle: "set_threshold"
---

Set a runtime XP threshold as a wizard. Defaults return after database reload.
Existing earned levels are recalculated on the next accepted award.

## Signature

```lua
btech.character.set_threshold(player, skill, threshold)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `player` | `integer` | Wizard actor. |
| `skill` | `string` | Canonical name or short alias. |
| `threshold` | `integer` | From 0 through 2147483647; zero disables earned levels. |

## Returns

- `boolean`
