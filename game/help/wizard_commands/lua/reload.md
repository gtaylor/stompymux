+++
title = "@lua/reload"
description = "Reload all Lua modules atomically"
keywords = ["@lua/reload"]
article_tags = ["lua_switches"]
weight = 30
wizard_only = true
+++

# @lua/reload

```text
@lua/reload
```

Capture object logic, global logic and helper packages from disk, then construct
a separate candidate runtime. New files become available; deleted modules leave
the catalog. Missing attached parents, invalid declarations, callback/resource
errors or failed persistence leave the active runtime and pending jobs intact.

Module initialization may change world state or queue output. These changes are
validated and saved before publication; output follows successful persistence.
Lua globals reset. Bootstrap, startup and connect hooks are not run again.

Successful reload cancels pending schedules from the old runtime. The observed
minute remains recorded, so reload neither recollects that minute nor backfills
missed work. New definitions apply on subsequent newly observed minutes.

Sessions, terminal negotiation, help metadata and player macros remain intact.
