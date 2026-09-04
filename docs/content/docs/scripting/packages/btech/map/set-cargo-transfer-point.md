---
draft: true
title: set_cargo_transfer_point
type: docs
---

Atomically stores an in-bounds cargo-transfer point. Pass `nil` to clear it.

```lua
btech.map.set_cargo_transfer_point(map, { x = 4, y = 7, reveal_hint = true })
```
