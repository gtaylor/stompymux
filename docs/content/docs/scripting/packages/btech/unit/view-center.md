---
title: "btech.unit.view_center"
type: docs
linkTitle: "view_center"
manualLinkTitle: "view_center"
---

Resolve display centering only; this does not render or disclose terrain or occupants.
Arguments are empty, a contact label/dbref, or integer bearing and signed distance.

## Signature

```lua
btech.unit.view_center(dbref, pilot, kind, arguments)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `kind` | `'tactical'\|'long_range'` |  |
| `arguments` | `string?` |  |

## Returns

- `BattleViewPosition`
