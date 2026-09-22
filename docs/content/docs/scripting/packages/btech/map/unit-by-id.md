---
title: "btech.map.unit_by_id"
type: docs
linkTitle: "unit_by_id"
manualLinkTitle: "unit_by_id"
---

Resolve the first unit matching a two-character battlefield ID from a unit or map origin.

## Signature

```lua
btech.map.unit_by_id(origin, id)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `origin` | `DbRef\|Object` | Registered unit or map. |
| `id` | `string` | Exactly two ASCII characters. |

## Returns

- `Object|nil unit`
