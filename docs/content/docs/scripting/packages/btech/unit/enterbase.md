---
title: "btech.unit.enterbase"
type: docs
---

Begin an eighteen-second hangar entry; current route, eligibility and locks are rechecked at expiry.

## Signature

```lua
btech.unit.enterbase(dbref, pilot, direction)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `direction` | `string?` | One-byte direction; omitted or longer selector uses the first entrance. |

## Returns

- `boolean admitted False when the enter lock denies entry.`
