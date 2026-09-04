---
draft: true
title: show_weapon_specs_ref
type: docs
toc_hide: false
---

Requests a unit template's weapon-specification display for a player.

## Function

### Synopsis

```lua
btech.unit.show_weapon_specs_ref( reference, player )
```

### Arguments

`string reference`
: The unit template reference.

`number player`
: The recipient player dbref.

### Returns

`"1" success`
: The literal string `"1"` once the template and recipient pass validation. It
  does not confirm that the renderer produced a display.

## Notes

This function is available only in a running Lua callback. The reference is
resolved from the configured unit-template database. Lua-bridge conversion
of a non-scalar argument raises an ordinary Lua type error; exceeding the
bridge's argument limit raises `mux.arg.invalid`. An invalid template or
recipient raises `btech.failed`. After those checks, the renderer's result is
not reported; a template with no reportable weapons can produce no display and
still return `"1"`.

## See Also

- [`btech`](../../)
