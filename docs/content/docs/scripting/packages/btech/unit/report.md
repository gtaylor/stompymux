---
title: "btech.unit.report"
type: docs
---

Return a silent brief report of an acquired visible unit, with no armor or weapon details.
Requires a conscious assigned pilot, running unit and working scanners. Direct reports
do not impose the detailed scan radius. No dice, contacts or output are changed.

## Signature

```lua
btech.unit.report(dbref, pilot, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `target` | `integer` | Target unit dbref. |

## Returns

- `string Styled identity, motion and position summary.`
