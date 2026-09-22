---
title: "btech.unit.c3i_targets"
type: docs
linkTitle: "c3i_targets"
manualLinkTitle: "c3i_targets"
---

Inspect direct and network sightings without acquiring contacts or publishing output.

## Signature

```lua
btech.unit.c3i_targets(dbref, pilot)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |

## Returns

- `{rows: BattleNetworkTargetRow[], text: string}|nil`
- `table|nil error`
