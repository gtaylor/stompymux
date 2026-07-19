+++
title = "@lua/parent"
description = "Attach or clear an object's Lua parent"
keywords = ["@lua/parent"]
article_tags = ["lua_switches"]
weight = 20
wizard_only = true
+++

# @lua/parent

Attach an object-logic module to an object, or omit the path to clear the
attachment:

```text
@lua/parent <object>=<path>.lua
@lua/parent <object>=
```

Paths are relative to `game/lua/object_logic`. The closest attachment in an
object's normal MUX parent chain supplies its effective Lua parent.
