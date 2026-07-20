+++
title = "@examine"
description = "Inspect an object's complete internal state"
keywords = ["@examine", "@examine/brief", "@examine/debug", "@examine/parent"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @examine

Inspect an object as a Wizard:

```text
@examine [<object>]
@examine/brief [<object>]
@examine/debug <object>
@examine/parent <object>[/<attribute pattern>]
```

The normal view includes ownership, flags, powers, attributes, contents,
locations and links, plus the effective Lua parent's commands, events,
schedules, messages, and locks. `/brief` omits the ordinary attribute list,
`/debug` displays raw database fields, and `/parent` includes inherited
attributes when examining an attribute pattern.
