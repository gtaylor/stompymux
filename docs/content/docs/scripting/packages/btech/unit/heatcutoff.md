---
title: "btech.unit.heatcutoff"
type: docs
linkTitle: "heatcutoff"
manualLinkTitle: "heatcutoff"
---

Shut off that many of a Mech's heat sinks (zero switches them all back on); requires the
assigned conscious pilot. The change takes effect at the unit's next turn boundary
(Total Warfare p. 159, Shutting Off Heat Sinks).

## Signature

```lua
btech.unit.heatcutoff(dbref, pilot, sinks)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `sinks` | `integer` | Heat sinks to shut off, at most the working sinks. |

## Returns

- `boolean`
