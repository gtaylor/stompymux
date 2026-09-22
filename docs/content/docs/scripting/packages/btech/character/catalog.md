---
title: "btech.character.catalog"
type: docs
linkTitle: "catalog"
manualLinkTitle: "catalog"
---

Return ordered value definitions of one kind; a supplied player filters unsaved
skills and advantages while attributes stay complete.

## Signature

```lua
btech.character.catalog(kind, character)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `string` | Char_value, Char_skill, Char_advantage or Char_attribute. |
| `character?` | `DbRef\|Object` |  |

## Returns

- `BattleCharacterValueDefinition[] definitions`
