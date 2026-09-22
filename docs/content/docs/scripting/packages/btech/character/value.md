---
title: "btech.character.value"
type: docs
linkTitle: "value"
manualLinkTitle: "value"
---

Read one character value; skills additionally report target and experience progress.

## Signature

```lua
btech.character.value(character, value)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `character` | `DbRef\|Object` | Player object. |
| `value` | `string\|integer` | Character-value name, prefix or code. |

## Returns

- `BattleCharacterValueReport result`
