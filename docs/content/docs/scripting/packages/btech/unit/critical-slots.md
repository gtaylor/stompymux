---
title: critical_slots
type: docs
toc_hide: false
---

Lists a live unit section's critical slots.

## Function

### Synopsis

```lua
btech.unit.critical_slots( unit, section )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`string section`
: A class-specific section name or abbreviation.

### Returns

`BtechCriticalSlot[] slots`
: The critical-slot records.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
