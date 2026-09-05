---
title: experience_threshold
type: docs
toc_hide: false
---

Returns the configured base experience threshold for a skill.

## Function

### Synopsis

```lua
btech.character.experience_threshold( skill )
```

### Arguments

`string skill`
: The skill name.

### Returns

`integer threshold`
: The configured base threshold. This is not the character-specific experience
  remaining until the next level; use
  [`btech.character.value`](../value/)'s `experience_to_next_level` field for
  that value.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
