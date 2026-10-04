---
title: "btech.map.conditions"
type: docs
linkTitle: "conditions"
manualLinkTitle: "conditions"
---

Change saved light/weather conditions without reloading occupied terrain.
Perception follows the new light and visibility on the next scan; contacts and locks remain until then.

## Signature

```lua
btech.map.conditions(dbref, light, visibility)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Map object dbref. |
| `light` | `LightLevel` | Typed constant from btech.map.light_levels. |
| `visibility` | `integer` | Weather range from 0 through 60. |

## Returns

- `boolean`
