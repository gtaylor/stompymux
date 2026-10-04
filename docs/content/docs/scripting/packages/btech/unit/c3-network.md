---
title: "btech.unit.c3_network"
type: docs
linkTitle: "c3_network"
manualLinkTitle: "c3_network"
---

Inspect classic C3 peers using active master capacity; emits no messages.

## Signature

```lua
btech.unit.c3_network(dbref, pilot)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |

## Returns

- `{rows: NetworkStatusRow[], text: string}|nil`
- `table|nil error`
