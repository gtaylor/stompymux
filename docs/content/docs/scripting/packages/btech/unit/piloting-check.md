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
: The modifier applied to the unit's piloting skill roll, from -2,147,483,648
  through 2,147,483,647.

`integer damage_modifier`
: The modifier applied to the fall when the piloting check fails, from
  -2,147,483,648 through 2,147,483,647.

### Returns

`boolean succeeded`
: Whether the check succeeded.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
