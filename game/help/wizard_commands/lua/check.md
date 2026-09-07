+++
title = "@lua/check"
description = "Validate all Lua modules"
keywords = ["@lua/check"]
article_tags = ["lua_switches"]
weight = 10
wizard_only = true
+++

# @lua/check

```text
@lua/check
```

Validate captured object logic, global logic and shared package sources in an
isolated VM. Checks include syntax, module return values, required imports,
attached parent paths, command declarations, handlers, locks, schedules and test
suite declarations. Test hooks and bodies are not executed.
Unused packages are syntax-checked; required packages are evaluated normally.

World, session, state and channel operations are unavailable during checking.
Pure text/configuration helpers and typed constants remain available. Consequently
a module that initializes world state can reload successfully but fail checking.
Errors identify the source module. Neither success nor failure replaces the
active runtime, modifies the database or sends candidate module output.
