---
title: apply_damage
type: docs
toc_hide: false
---

Applies clustered damage to a live unit.

## Function

### Synopsis

```lua
btech.unit.apply_damage( unit, request )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`table request`
: The damage request.

The request requires these fields:

`integer amount`
: The total damage to apply, from 1 to 1000.

`integer cluster_size`
: The maximum damage in each cluster, from 1 to 1000.

`integer direction_code`
: The attack direction code, from 0 to 21.

Codes 0 through 7 damage a fixed section; codes 8 through 15 damage the rear
armor of the corresponding section (`code - 8`). Section indices depend on the
unit class:

| Indices | Unit class and section order |
| --- | --- |
| 0–7 | Biped mech: left arm, right arm, left torso, right torso, center torso, left leg, right leg, head. |
| 0–7 | Quad mech: front left leg, front right leg, left torso, right torso, center torso, rear left leg, rear right leg, head. |
| 0–7 | Battle suit: suit members 1 through 8. |
| 0–5 | Ground/naval vehicle or VTOL: left side, right side, front side, aft side, turret, rotor. |
| 0–3 | Aerospace unit: nose, left wing, right wing, aft. |
| 0–5 | Aerodyne DropShip: right wing, left wing, left rear wing, right rear wing, aft, nose. |
| 0–5 | Spheroid DropShip: front right, front left, rear left, rear right, aft, nose. |

Codes 16 through 21 choose a random section using the unit class's hit-location
table:

| Code | Hit-location table | Rear armor |
| --- | --- | --- |
| 16 | Rear | No |
| 17 | Left side | No |
| 18 | Right side | No |
| 19 | Front | Yes |
| 20 | Rear | Yes |
| 21 | Left side | Yes |

It also accepts these optional fields:

`boolean force_critical`
: Whether to force a critical-hit check.

`string unit_message`
: A message sent to the damaged unit.

`string map_message`
: A message broadcast on the unit's map.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
