---
title: piloting_check
type: docs
toc_hide: false
---

Makes a piloting check and applies a failed result.

## Function

### Synopsis

```lua
btech.unit.piloting_check( unit, options )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`table options`
: The piloting-check options.

The options table requires these fields:

`integer roll_modifier`
: The modifier applied to the unit's piloting skill roll. Use a gameplay
  modifier from -100 through 100; values outside this range are unsupported.

`integer damage_modifier`
: The number of fall levels applied when the piloting check fails. Use a value
  from 0 through 100; values outside this range are unsupported because the
  fall-damage calculation and damage application are not designed for them.

### Returns

`boolean succeeded`
: Whether the check succeeded.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
