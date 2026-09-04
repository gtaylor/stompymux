---
draft: true
title: set_preferred_id
type: docs
---

Sets a registered unit's preferred tactical ID. The value must be two ASCII
letters and is normalized to uppercase; pass `nil` to clear it.

```lua
btech.unit.set_preferred_id(unit, "BK")
```
