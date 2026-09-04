---
title: damage
type: docs
toc_hide: false
---

Applies clustered damage to a live unit.

## Function

### Synopsis

```lua
btech.unit.damage( unit, damage, cluster_size, direction, force_critical, unit_message, los_message )
```

### Arguments

`number unit`
: The unit dbref.

`integer damage`
: Total damage, from 1 through 1000.

`integer cluster_size`
: Damage applied per cluster; at least 1.

`integer direction`
: The attack direction code.

`boolean or integer force_critical`
: Whether to try to force a critical hit.

`string unit_message`
: Message sent to the damaged unit.

`string los_message`
: Message broadcast to units with line of sight.

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
