---
title: "btech.unit.dfa"
type: docs
linkTitle: "dfa"
manualLinkTitle: "dfa"
---

Attempt a DFA jump using the shared pre-launch stagger check. Nil uses the current target lock.

## Signature

```lua
btech.unit.dfa(dbref, pilot, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `target?` | `integer` |  |

## Returns

- `boolean Accepted attempt; a stagger failure can prevent launch.`
