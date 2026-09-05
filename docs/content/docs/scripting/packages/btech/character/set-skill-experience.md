---
title: set_skill_experience
type: docs
toc_hide: false
---

Sets a character skill's packed experience value.

## Function

### Synopsis

```lua
btech.character.set_skill_experience( character, skill, experience )
```

### Arguments

`Object character`
: The character.

`string skill`
: The skill name.

`integer experience`
: The new packed experience value, from 0 through 2,147,483,647. The value
  modulo 16,777,216 is the skill's accumulated experience; each complete
  multiple of 16,777,216 adds one to the skill's effective value.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
