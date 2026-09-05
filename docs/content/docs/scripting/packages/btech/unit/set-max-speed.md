---
title: set_max_speed
type: docs
toc_hide: false
---

Sets a live unit's maximum speed.

## Function

### Synopsis

```lua
btech.unit.set_max_speed( unit, speed )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`number speed`
: The new maximum speed, from 0 to 100000.

### Returns

None.

## Notes

After setting the limit, this function corrects the unit's movement state. If
necessary, it immediately clamps the desired and current speeds to the new
maximum and recalculates cargo weight.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
