---
title: "btech.unit.push"
type: docs
linkTitle: "push"
manualLinkTitle: "push"
---

Push the standing Mech directly ahead with both arms. A hit displaces it one hex onward, the
attacker follows into the hex it left, and the target must keep its footing; no damage is dealt.

## Signature

```lua
btech.unit.push(dbref, pilot, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `target?` | `integer` | Defaults to the selected target; explicit targets require acquisition. |

## Returns

- `table report Push profile, roll, hit, displacement, balance checks and notices.`
