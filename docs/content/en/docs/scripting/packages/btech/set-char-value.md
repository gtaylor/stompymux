---
title: btech.set_char_value
linkTitle: btech.set_char_value
type: docs
weight: 252
---

# `btech.set_char_value`

Sets a character value or adjusts skill experience.

## Function

### Synopsis

```lua
btech.set_char_value( character, value, amount, mode )
```

### Arguments

`number or string character`
: The character dbref or player name.

`number or string value`
: The character-value code or name.

`number amount`
: The value or experience amount.

`number mode`
: `0` sets level/value, `1` sets skill target, `3` sets XP, and other nonzero values add XP.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
