---
title: "btech.unit.viewport"
type: docs
linkTitle: "viewport"
manualLinkTitle: "viewport"
---

Resolve display bounds only, without rendering or disclosing terrain/occupants.

## Signature

```lua
btech.unit.viewport(dbref, pilot, kind, arguments, dimensions)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `kind` | `'tactical'\|'long_range'` |  |
| `arguments` | `string?` | Own unit, target label/dbref, or bearing and distance. |
| `dimensions` | `BattleViewDimensions?` |  |

## Returns

- `BattleViewport`
