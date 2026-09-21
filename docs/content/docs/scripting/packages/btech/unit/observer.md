---
title: "btech.unit.observer"
type: docs
---

Inspect or set observer role. Trusted scripts own authorization; cockpit pilots cannot grant this role.

## Signature

```lua
btech.unit.observer(dbref, enabled)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `enabled` | `boolean?` | Omit to inspect the saved role. |

## Returns

- `boolean`
