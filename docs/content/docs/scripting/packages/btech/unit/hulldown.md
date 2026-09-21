---
title: "btech.unit.hulldown"
type: docs
---

Lower a quad, raise it with "-", or cancel its pending change with "stop".

## Signature

```lua
btech.unit.hulldown(dbref, pilot, argument)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` | Conscious assigned pilot; scripts own authority to act for them. |
| `argument` | `string?` | Omit to lower. |

## Returns

- `boolean`
