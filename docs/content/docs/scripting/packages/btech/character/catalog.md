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
: The value kind to list.

`Object|nil character`
: An optional character used to resolve character-specific definitions.

### Returns

`BtechCharacterValueDefinition[] definitions`
: The matching value definitions.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
