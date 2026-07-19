+++
title = "@lua/schedule"
description = "Inspect active Lua schedules"
keywords = ["@lua/schedule"]
article_tags = ["lua_switches"]
weight = 40
wizard_only = true
+++

# @lua/schedule

List scheduled modules, or inspect the schedules for an object or module:

```text
@lua/schedule
@lua/schedule <object>
@lua/schedule <object_logic path>
@lua/schedule global_logic/<path>.lua
```

Object-logic paths are relative to `game/lua/object_logic`. Inspecting one also
lists the objects that currently inherit it.
