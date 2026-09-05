---
title: set_ui_preferences
type: docs
toc_hide: false
---

Sets or clears a player's map-display preferences.

## Function

### Synopsis

```lua
btech.player.set_ui_preferences( player, preferences )
```

### Arguments

`DbRef|Object player`
: The player.

`BtechUiPreferencesState|nil preferences`
: The new preferences, or `nil` to clear them and restore the defaults.

### Returns

None.

## Example

```lua
btech.player.set_ui_preferences(player, {
  tactical_height = 14, tactical_width = 21, lrs_height = 11,
  include_dead = false, include_shutdown = true,
  include_enemies = true, include_allies = true, include_target = true,
  buildings = "exclude",
})
```

## Notes

Heights must be 5–24 for tactical, 5–40 for tactical width, and 10–40 for LRS.
A table returned by `ui_preferences` can be passed directly; its informational
`configured` field is ignored.

## See Also

- [`btech`](../../)
- [`btech.player`](../)
- [`btech.player.ui_preferences`](../ui-preferences/)
