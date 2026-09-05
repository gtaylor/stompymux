---
title: set_skill_target
type: docs
toc_hide: false
---

Sets a character skill's target number.

## Function

### Synopsis

```lua
btech.character.set_skill_target( character, skill, target )
```

### Arguments

`Object character`
: The character.

`string skill`
: The skill name.

`integer target`
: A target number representable by a raw skill level from 0 through 255. The
  reachable target range depends on the character's base target and current
  skill modifiers.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
