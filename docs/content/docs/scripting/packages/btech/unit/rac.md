---
title: "btech.unit.rac"
type: docs
linkTitle: "rac"
manualLinkTitle: "rac"
---

Set rotary burst length; repeated selection stays enabled and returns false.

## Signature

```lua
btech.unit.rac(dbref, pilot, weapon, rounds)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `weapon` | `integer` | Zero-based weapon index. |
| `rounds?` | `1\|2\|3\|4\|5\|6` | Defaults to one. |

## Returns

- `boolean changed`
