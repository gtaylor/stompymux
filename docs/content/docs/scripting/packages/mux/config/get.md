---
title: get
type: docs
toc_hide: false
---

Returns the current value of a scalar server configuration directive.

## Function

### Synopsis

```lua
mux.config.get( name )
```

### Arguments

`string name`
: The exact, case-sensitive directive name, such as `mud_name`,
  `player_name_spaces`, or `btech_techtime_multiplier`.

### Returns

The live configured value as a Lua `string`, `number`, or `boolean`. Integer
database references remain numbers. `lua_error_reporting` is returned as
`"off"`, `"wizards"`, or `"all"`.

### Errors

- `mux.arg.invalid` when the name contains an embedded NUL byte.
- `mux.config.not_found` when no directive has that exact name.
- `mux.config.unsupported` when the directive exists but is not a readable
  scalar.

## Examples

```lua
local game_name = mux.config.get("mud_name")
local names_allow_spaces = mux.config.get("player_name_spaces")
local repair_multiplier = mux.config.get("btech_techtime_multiplier")
```

## Notes

Values are read on every call and are not cached. Lua modules are trusted
server logic, so command-level configuration permissions do not restrict this
API. The function is available during `@lua/check`.
