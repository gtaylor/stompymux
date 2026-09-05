---
title: set_value
type: docs
toc_hide: false
---

Sets a script-visible character value.

## Function

### Synopsis

```lua
btech.character.set_value( character, value, amount )
```

### Arguments

`Object character`
: The character.

`string value`
: The value name.

`integer amount`
: The new value. Values below 0 are stored as 0, and values above 255 are stored
  as 255.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
