---
title: list
type: docs
---

Returns the currently set flags as `Flag[]` constants in native registry order.
An object with no flags returns an empty array.

```lua
for _, flag in ipairs(object:flags():list()) do
  mux.log("flags.log", tostring(flag))
end
```
