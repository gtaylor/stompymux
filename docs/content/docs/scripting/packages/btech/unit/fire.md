---
title: "btech.unit.fire"
type: docs
linkTitle: "fire"
manualLinkTitle: "fire"
---

Fire under configured tactical rules and stage cockpit notices in the current callback transaction.
Requires the conscious assigned pilot; the calling script owns authority to act as that pilot.
An omitted target uses cockpit selection, including automatic coolant self-selection.
An IDF observer takes precedence when the firer has no unit lock, even with explicit arguments.
Explicit coordinates select an occupant for conventional weapons or use terrain effects when empty.
Artillery always uses coordinates and queues its impact. Explicit requests do not change saved locks.
Missile effects require the base target number even with near-miss glancing enabled.
Such a near miss spends its launch and aimed preparation, but starts no AMS, pod effect or Swarm flight.
Results are detached; optional fields use nil.

## Signature

```lua
btech.unit.fire(dbref, pilot, weapon, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `weapon` | `integer` | Zero-based weapon number from unit.weapons. |
| `target` | `integer\|{x: integer, y: integer}\|nil` | Explicit target leaves the selected lock unchanged. |

## Returns

- `MechShotReport|VehicleShotReport|HexShotReport|ArtilleryLaunchReport`
