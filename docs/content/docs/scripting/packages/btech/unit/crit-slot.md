---
title: crit_slot
type: docs
toc_hide: false
---

Describes one critical slot on a live unit.

## Function

### Synopsis

```lua
btech.unit.crit_slot( unit, section, slot, field )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: A full section name, matched without regard to case. Otherwise the legacy
  resolver uses a class-dependent one- or two-character prefix and may ignore
  trailing characters.

`number slot`
: The critical-slot number.

`string field`
: Required field selector: `NAME`, `STATUS`, `DATA`, `MAXAMMO`, `AMMOTYPE`,
  `MODE`, or `HALFTON`. Matching is case-insensitive. An unrecognized supplied
  selector currently falls back to `NAME`.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets,
invalid arguments, and legacy error results raise a Lua error. Although the
legacy handler's count check permits three arguments, `field` must be supplied:
the current wrapper unconditionally reads it.

## See Also

- [`btech`](../../)
