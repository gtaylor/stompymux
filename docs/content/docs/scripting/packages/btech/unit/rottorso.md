---
title: "btech.unit.rottorso"
type: docs
---

Rotate one step left/right or center the torso; stages the native cockpit message.

## Signature

```lua
btech.unit.rottorso(dbref, pilot, direction)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` | Conscious assigned pilot; scripts own authority to act for them. |
| `direction` | `'left'\|'right'\|'center'` | Case-insensitive; l/r/c aliases are accepted. |

## Returns

- `boolean`
