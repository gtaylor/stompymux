---
title: catalog
type: docs
toc_hide: false
---

Lists configured character values of a given kind.

## Function

### Synopsis

```lua
btech.character.catalog( kind, character )
```

### Arguments

`string kind`
: One of `Char_value`, `Char_skill`, `Char_advantage`, or `Char_attribute`,
  matched case-insensitively. An unrecognized kind returns an empty array.

`Object|nil character`
: An optional character used to restrict non-attribute results to values whose
  current amount or experience is nonzero. Attributes are always retained.

### Returns

`BtechCharacterValueDefinition[] definitions`
: The matching value definitions.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
