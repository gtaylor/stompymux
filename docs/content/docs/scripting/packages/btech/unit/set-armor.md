---
title: set_armor
type: docs
toc_hide: false
---

Updates selected armor fields for one live unit section.

## Function

### Synopsis

```lua
btech.unit.set_armor( unit, section, patch )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`string section`
: A class-specific section name or abbreviation.

`table patch`
: One or more of `armor`, `internal`, and `rear_armor`.

### Returns

None.

## Notes

Every supplied value must be an integer from 0 through 255.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
