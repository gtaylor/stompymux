---
title: "btech.autopilot.feedback"
type: docs
linkTitle: "feedback"
manualLinkTitle: "feedback"
---

Read bounded order transitions and outcome records.

## Signature

```lua
btech.autopilot.feedback(unit, after_sequence)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `integer` | The controlled unit. |
| `after_sequence` | `integer` | Optional sequence cursor. |

## Returns

- `BattleAutopilotFeedbackPage Feedback records and a history-gap indicator.`
