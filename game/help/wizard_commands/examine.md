+++
title = "@examine"
description = "Inspect an object's complete internal state"
keywords = ["@examine", "@examine/brief", "@examine/debug"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @examine

Inspect an object as a Wizard:

```text
@examine [<object>]
@examine/brief [<object>]
@examine/debug <object>
@examine <object>[/<attribute pattern>]
```

The normal view includes ownership, flags, powers, Lua storage entries, contents,
locations and links, plus the direct Lua parent's appearances, commands,
events, schedules, messages, and locks. `/brief` omits the ordinary attribute list,
and `/debug` displays raw database fields. Storage names and patterns are
case-sensitive. Entries have no flags, owners, or parent inheritance.
