---
title: btech.damage_mech
type: docs
toc_hide: false
---

Applies clustered damage to a live unit.

## Function

### Synopsis

```lua
btech.damage_mech( unit, damage, cluster_size, direction, force_critical, unit_message, los_message )
```

### Arguments

`number unit`
: The unit dbref.

`number damage`
: Total damage, from 1 through 1000.

`number cluster_size`
: Damage applied per cluster; at least 1.

`number direction`
: The attack direction code.

`boolean or number force_critical`
: Whether to try to force a critical hit.

`string unit_message`
: Message sent to the damaged unit.

`string los_message`
: Message broadcast to units with line of sight.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
