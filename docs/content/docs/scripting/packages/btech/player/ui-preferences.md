---
title: ui_preferences
type: docs
toc_hide: false
---

Returns a player's effective map-display preferences.

## Function

### Synopsis

```lua
btech.player.ui_preferences( player )
```

### Arguments

`DbRef|Object player`
: The player.

### Returns

`BtechUiPreferencesState preferences`
: The effective preferences and configured state.

## Example

```lua
local preferences = btech.player.ui_preferences(player)
if preferences.configured then
  -- The player has explicitly configured these values.
end
```

## Notes

The table contains `tactical_height`, `tactical_width`, `lrs_height`, the five
`include_*` booleans, `buildings` (`follow_brief`, `include`, or `exclude`), and
the `configured` boolean.

## See Also

- [`btech`](../../)
- [`btech.player`](../)
- [`btech.player.set_ui_preferences`](../set-ui-preferences/)
