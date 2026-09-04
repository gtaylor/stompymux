---
title: make_pilot_roll
type: docs
toc_hide: false
---

Makes a piloting roll and causes a fall when it fails.

## Function

### Synopsis

```lua
btech.unit.make_pilot_roll( unit, roll_modifier, damage_modifier )
```

### Arguments

`number unit`
: The unit dbref.

`integer roll_modifier`
: Modifier applied to the piloting roll.

`integer damage_modifier`
: Modifier passed to falling damage.

### Returns

`boolean result`
: Whether the condition is true.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
