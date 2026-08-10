---
title: btech.char_value
type: docs
toc_hide: false
---

Gets a character attribute, skill level, target, experience, or experience threshold.

## Function

### Synopsis

```lua
btech.char_value( character, value, mode )
```

### Arguments

`number or string character`
: The character dbref or player name.

`number or string value`
: The character-value code or name.

`number mode`
: `0` for value, `1` for skill target, `2` for XP, `3` for raw skill value, or `4` for XP to next level.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
