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
```

The normal view includes flags, powers, contents, locations and links, plus the
object's zone and affiliation and the direct Lua parent's appearances,
commands, events, schedules, messages, and locks. Persistent object state is
summarized by namespace, with the number of values in each namespace; keys and
values are not displayed. `/brief` omits the namespace summary, and `/debug`
displays raw database fields and the total number of persistent Lua state
entries. Normal `@examine` never displays state keys or values.

Registered BattleTech objects also show `BTech type: TYPE`. This line appears
in normal, `/brief`, and `/debug` output and is omitted for unregistered
objects.

The examined object's name, description, and internal description are shown
using editable styled text markup instead of terminal color escape sequences.
The internal description is omitted when
it is empty. This output can be copied into `@name`, `@description`, or
`@internal-description`.

Only Wizards can use `@examine`. Wizards may examine any object.
