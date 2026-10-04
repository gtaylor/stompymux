---
title: "btech.cargo.manifest"
type: docs
linkTitle: "manifest"
manualLinkTitle: "manifest"
---

Read detached stock in the actor's current location. Requires cargo commands enabled.

## Signature

```lua
btech.cargo.manifest(actor, pattern)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `pattern?` | `string` | Case-insensitive stock name, numeric part ID, or wildcard pattern. |

## Returns

- `CargoRow[]`
