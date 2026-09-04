---
draft: true
title: zone_units
type: docs
toc_hide: false
---

Lists live unit objects assigned to a zone.

## Function

### Synopsis

```lua
btech.system.zone_units( zone )
```

### Arguments

`number zone`
: The zone dbref.

### Returns

`table values`
: A flat array of unit dbrefs. If the legacy output buffer fills, the final value
  is `-1` to indicate that the list was truncated.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
