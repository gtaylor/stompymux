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
  remaining until the next level. The `experience_to_next_level` field returned
  by [`btech.character.value`](../value/) is the character's cumulative target;
  subtract the returned `experience` field to calculate the remaining amount.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
