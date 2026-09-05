---
title: armor
type: docs
toc_hide: false
---

Returns a template's armor and internal values.

## Function

### Synopsis

```lua
btech.template.armor( reference, section )
```

### Arguments

`string reference`
: The unit-template reference.

`string|nil section`
: An optional class-specific section name or abbreviation.

### Returns

`BtechArmorStatus status`
: The requested armor status.

## Notes

When `section` is omitted, the returned armor, internal-structure, and rear-
armor values are totals across all sections, and the record has no `section`
field.

## See Also

- [`btech`](../../)
- [`btech.template`](../)
