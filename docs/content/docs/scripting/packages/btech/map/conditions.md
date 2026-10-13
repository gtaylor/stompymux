---
title: "btech.map.conditions"
type: docs
linkTitle: "conditions"
manualLinkTitle: "conditions"
---

Change saved light and weather conditions without reloading occupied terrain.
Perception follows the new conditions on the next scan; contacts and locks remain until then.

## Signature

```lua
btech.map.conditions(dbref, light, weather)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Map object dbref. |
| `light` | `LightLevel` | Typed constant from btech.map.light_levels. |
| `weather` | `WeatherCondition` | Typed constant from btech.map.weather_conditions. |

## Returns

- `boolean`
