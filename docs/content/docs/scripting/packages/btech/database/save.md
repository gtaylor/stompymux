---
title: "btech.database.save"
type: docs
---

Request persistence of the current world at transaction commit, even if unchanged.

## Signature

```lua
btech.database.save(actor)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` | Wizard requesting the checkpoint. |

## Returns

- `boolean queued Success is published only after persistence; rollback cancels the request.`
