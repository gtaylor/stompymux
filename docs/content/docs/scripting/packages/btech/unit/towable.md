---
title: "btech.unit.towable"
type: docs
---

Inspect or set scenario permission to tow this unit out of character.
Trusted scripts own authorization for edits; this is not a pilot preference.
Disabling permission does not release an existing tow.

## Signature

```lua
btech.unit.towable(dbref, enabled)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `enabled` | `boolean?` | Omit to inspect without changing state. |

## Returns

- `boolean Current permission.`
