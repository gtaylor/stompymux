---
title: "btech.map.save"
type: docs
---

Stage an atomic asset replacement after world commit; true means queued, not written.

## Signature

```lua
btech.map.save(actor, dbref, name)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` | Wizard receiving the completion or failure notice. |
| `dbref` | `integer` |  |
| `name` | `string` | Relative name inside the configured map directory. |

## Returns

- `boolean`
