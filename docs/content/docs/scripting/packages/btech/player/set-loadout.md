---
title: set_loadout
type: docs
---

Atomically stores a personal-combat loadout. Pass `nil` to clear it. Equipment
has a required nonempty `weapon` and optional ammunition count from 0–255.

```lua
btech.player.set_loadout(player, {
  armor = { head = 2, torso = 8, hands = 2, feet = 2 },
  right = { weapon = "Laser Pistol", ammunition = 12 },
})
```
