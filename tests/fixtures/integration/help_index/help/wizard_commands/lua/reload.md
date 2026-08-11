+++
title = "@lua/reload"
description = "Reload all Lua modules atomically"
keywords = ["@lua/reload"]
article_tags = ["lua_switches"]
weight = 30
wizard_only = true
+++

# @lua/reload

Build and validate a replacement Lua state, then put it into service
atomically:

```text
@lua/reload
```

If any module cannot be loaded or validated, the current Lua state remains in
service.
