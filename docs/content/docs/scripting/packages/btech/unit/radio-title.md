---
title: "btech.unit.radio_title"
type: docs
linkTitle: "radio_title"
manualLinkTitle: "radio_title"
---

Save a title, truncated to fifteen bytes at a UTF-8 boundary. Transactional.

## Signature

```lua
btech.unit.radio_title(dbref, pilot, channel, title)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `channel` | `integer` | Zero-based channel (A is 0). |
| `title` | `string` | Empty clears the title. |

## Returns

- `boolean`
