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
lists the objects directly attached to it.

These reports show the declarations captured when the server started; they do not
execute modules or reload edited files. Restart to apply schedule changes.
`@lua` lists the implemented switches. Other Lua administration operations,
including reload, are not currently available.

Schedules use five-field numeric cron expressions in UTC. Matching jobs are
spread deterministically across the first 55 seconds of the minute. The startup
minute and missed minutes are skipped; jobs expire at the next minute boundary
and are not retried after failure. Each callback commits world changes before
sending output. Pending schedules are discarded during shutdown.

Reports are private to your session. Large reports indicate truncation; select
an individual module to narrow the results.
