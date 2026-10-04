---
title: "btech.tactical.submit"
type: docs
linkTitle: "submit"
manualLinkTitle: "submit"
---

Atomically validate and submit intentions for 1 to 100 distinct friendly controllers.
Any invalid order or stale revision rejects the whole batch. Paused units stay paused.

## Signature

```lua
btech.tactical.submit(intentions)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `intentions` | `TacticalIntention[]` | One intention per unit; revisions are required. |

## Returns

- `TacticalSubmitResult[] results Assigned IDs and revisions in request order.`
