---
title: "mux.macro.create_set"
type: docs
linkTitle: "create_set"
manualLinkTitle: "create_set"
---

Create a private unlocked set without attaching it.
Requires an active callback transaction; bypasses player permission checks.

## Signature

```lua
mux.macro.create_set(owner, description)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `owner` | `DbRef\|Object` |  |
| `description` | `string` |  |

## Returns

- `MacroSet set`
