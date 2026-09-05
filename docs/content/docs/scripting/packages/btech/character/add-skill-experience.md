---
title: add_skill_experience
type: docs
toc_hide: false
---

Adds experience to one of a character's skills.

## Function

### Synopsis

```lua
btech.character.add_skill_experience( character, skill, amount )
```

### Arguments

`Object character`
: The character.

`string skill`
: The skill name.

`integer amount`
: The signed experience adjustment. The character's current packed experience
  plus this amount must remain within -2,147,483,648 through 2,147,483,647;
  exceeding that range is unsupported.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.character`](../)
