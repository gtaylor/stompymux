---
title: "btech.autopilot.configure"
type: docs
linkTitle: "configure"
manualLinkTitle: "configure"
---

Atomically update controller settings and return the new management revision.

## Signature

```lua
btech.autopilot.configure(unit, patch, expected_revision)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `integer` | The controlled unit. |
| `patch` | `BattleAutopilotConfigPatch` | Settings to change. |
| `expected_revision` | `integer` | Optional revision guard. |

## Returns

- `integer New management revision.`
