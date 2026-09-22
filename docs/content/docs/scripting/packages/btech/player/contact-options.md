---
title: "btech.player.contact_options"
type: docs
linkTitle: "contact_options"
manualLinkTitle: "contact_options"
---

Decode transient unit-list options d/s/e/a/t and persistent exclusion prefix !.
Does not access game state; b requests building identification in native output.

## Signature

```lua
btech.player.contact_options(options, brief_buildings)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `options` | `string` | Single word, up to fifty characters are processed. |
| `brief_buildings` | `boolean?` | Initial building inclusion from unit brief settings; defaults false. |

## Returns

- `BattleContactOptions`
