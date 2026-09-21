---
title: "btech.unit.scan"
type: docs
---

Inspect an acquired visible target without changing contacts or consuming dice.
Stages a warning to running targets unless the scanning unit is an observer.
Requires the conscious assigned pilot, a running unit and operational scanners.
Observers bypass distance and receive exact status; ordinary scans disclose condition bands.

## Signature

```lua
btech.unit.scan(dbref, pilot, target, options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `target` | `integer` | Target unit dbref. |
| `options` | `string?` | A (armor), I (info), W (weapons), or a combination; omitted means all. |

## Returns

- `string Styled scan report.`
