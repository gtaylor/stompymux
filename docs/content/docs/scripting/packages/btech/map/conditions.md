---
title: "btech.map.conditions"
type: docs
linkTitle: "conditions"
manualLinkTitle: "conditions"
---

Change saved light/weather conditions without reloading occupied terrain.
Changed light rechecks active sensors and publishes cockpit warnings transactionally; locks and pending requests remain.

## Signature

```lua
btech.map.conditions(dbref, light, visibility)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Map object dbref. |
| `light` | `"night"\|"twilight"\|"day"` |  |
| `visibility` | `integer` | Weather range from 0 through 60. |

## Returns

- `boolean`
