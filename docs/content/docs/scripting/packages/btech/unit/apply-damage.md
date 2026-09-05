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
