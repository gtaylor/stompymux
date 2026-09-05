---
title: value
type: docs
toc_hide: false
---

Returns one script-visible character value.

## Function

### Synopsis

```lua
btech.character.value( character, value )
```

### Arguments

`Object character`
: The character.

`string|integer value`
: The value name or numeric code.

### Returns

`BtechCharacterValue result`
: The definition and current value.

## Notes

`experience_to_next_level` is a cumulative experience target. Subtract the
result's current `experience` to calculate the experience still required.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
