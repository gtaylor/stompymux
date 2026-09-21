---
title: "btech.unit.visibility"
type: docs
---

Read or replace trusted scenario visibility. Both fields are required when replacing it.
Clairvoyance bypasses visibility checks; ordinary sensor acquisition still rejects invisible targets.

## Signature

```lua
btech.unit.visibility(dbref, flags)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `flags` | `{invisible: boolean, clairvoyant: boolean}?` |  |

## Returns

- `{invisible: boolean, clairvoyant: boolean}`
