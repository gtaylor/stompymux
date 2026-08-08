---
title: btech.make_pilot_roll
linkTitle: btech.make_pilot_roll
type: docs
weight: 236
---

# `btech.make_pilot_roll`

Makes a piloting roll and causes a fall when it fails.

## Function

### Synopsis

```lua
btech.make_pilot_roll( unit, roll_modifier, damage_modifier )
```

### Arguments

`number unit`
: The unit dbref.

`number roll_modifier`
: Modifier applied to the piloting roll.

`number damage_modifier`
: Modifier passed to falling damage.

### Returns

`boolean result`
: Whether the condition is true.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
