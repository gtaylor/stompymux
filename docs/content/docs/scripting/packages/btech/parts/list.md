---
title: list
type: docs
toc_hide: false
---

Lists canonical long part names in one category.

## Function

### Synopsis

```lua
btech.parts.list( category )
```

### Arguments

`string category`
: `"ammo"`, `"weapon"`, `"bomb"`, `"special"`, or `"cargo"`. The aliases
  `"weapons"`, `"weap"`, `"bombs"`, `"specials"`, `"part"`, `"parts"`, and
  `"carg"` are also accepted. All spellings are matched without regard to
  case.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Examples

```lua
local btech = require("btech")

for _, name in ipairs(btech.parts.list("weapon")) do
  mux.world.pemit(ctx.enactor, name)
end
```

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
- [`btech.parts.categories`](../categories/)
