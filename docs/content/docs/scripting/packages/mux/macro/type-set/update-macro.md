---
title: "MacroSet:update_macro"
type: docs
linkTitle: "update_macro"
manualLinkTitle: "update_macro"
---

Update expansion while retaining alias spelling; missing aliases raise mux.macro.not_found.
Requires an active callback transaction and a live handle.

## Signature

```lua
MacroSet:update_macro(alias, expansion)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `alias` | `string` |  |
| `expansion` | `string` |  |

## Returns

No values.
