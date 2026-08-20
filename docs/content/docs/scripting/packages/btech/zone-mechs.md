---
title: zone_mechs
type: docs
toc_hide: false
---

Lists live unit objects assigned to a zone.

## Function

### Synopsis

```lua
btech.zone_mechs( zone )
```

### Arguments

`number zone`
: The zone dbref.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
