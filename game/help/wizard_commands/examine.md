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

The normal view includes flags, powers, Lua storage entries, contents, locations
and links, plus the direct Lua parent's appearances, commands,
events, schedules, messages, and locks. `/brief` omits the ordinary attribute list,
and `/debug` displays raw database fields. Storage names and patterns are
case-sensitive. Entries have no flags or parent inheritance.

The examined object's name, `Desc`, and `Idesc` are shown using editable styled
text markup instead of terminal color escape sequences. `Idesc` is omitted when
it is empty. This output can be copied into `@name`, `@desc`, or `@idesc`.

Only Wizards can use `@examine`. Wizards may examine any object.
