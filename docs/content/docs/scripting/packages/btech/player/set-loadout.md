---
title: set_loadout
type: docs
toc_hide: false
---

Sets or clears a player's personal-combat loadout.

## Function

### Synopsis

```lua
btech.player.set_loadout( player, loadout )
```

### Arguments

`DbRef|Object player`
: The player.

`BtechPersonalCombatLoadout|nil loadout`
: The new loadout, or `nil` to clear it.

A loadout contains a required `armor` table:

| Field | Type | Range |
| --- | --- | --- |
| `head` | integer | 0 to 2 |
| `torso` | integer | 0 to 8 |
| `hands` | integer | 0 to 2 |
| `feet` | integer | 0 to 2 |

It may also contain `right` and `left` equipment tables. Each equipment table
has these fields:

`BtechPartRef weapon`
: A personal-combat weapon.

`integer|nil ammunition`
: Optional ammunition from 0 to 255. This field is valid only for a weapon that
  uses ammunition.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.player`](../)
