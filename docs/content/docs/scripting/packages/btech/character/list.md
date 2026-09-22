---
title: "btech.character.list"
type: docs
linkTitle: "list"
manualLinkTitle: "list"
---

List canonical names in catalog order. An optional player filters skills with nonzero value or XP.
Advantages and attributes remain complete when a player is supplied. Requires a callback.

## Signature

```lua
btech.character.list(kind, player)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `"skills"\|"advantages"\|"attributes"` | Full category names are case-insensitive; abbreviations are rejected. |
| `player?` | `integer\|string` | Live player id, name, account alias or #dbref; omit rather than passing explicit nil. |

## Returns

- `string[]`
