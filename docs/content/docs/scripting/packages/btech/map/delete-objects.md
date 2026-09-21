---
title: "btech.map.delete_objects"
type: docs
---

Delete map objects by type, coordinate, or both. At least one selector is required.

## Signature

```lua
btech.map.delete_objects(actor, dbref, kind, x, y)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `kind?` | `string` | FIRE, SMOKE, DECO, MINE, BUILDING, LEAVE, ENTRA, LINKED, or BLZ; prefixes accepted. |
| `x?` | `integer` | Must be paired with y. |
| `y?` | `integer` | Must be paired with x. |

## Returns

- `integer Number of selected records deleted; reciprocal cleanup is not counted.`
