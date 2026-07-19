+++
title = "@lua/check"
description = "Validate all Lua modules"
keywords = ["@lua/check"]
article_tags = ["lua_switches"]
weight = 10
wizard_only = true
+++

# @lua/check

Validate every Lua module without replacing the running Lua state:

```text
@lua/check
```

The check covers object logic, global logic, shared packages, configured Lua
parent paths, module return values, imports, and schedules. Runtime-only `mux`
operations are unavailable while validation evaluates module top-level code.
