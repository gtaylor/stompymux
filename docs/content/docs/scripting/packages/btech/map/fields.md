---
title: "btech.map.fields"
type: docs
---

Publish and return a wizard's map field report in catalogue order.

## Signature

```lua
btech.map.fields(actor, map, arguments)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `map` | `integer` |  |
| `arguments?` | `string` | Optional leading 1 or 4 selects columns, followed by a field prefix. |

## Returns

- `table report Map, columns, fields (name/value) and literal text. firstfree has no value.`
