+++
title = "@examine"
description = "Inspect an object's complete internal state"
keywords = ["@examine", "@examine/brief", "@examine/debug", "@entrances"]
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

BattleTech-specific inspection is not implemented.

The examined object's name, description, and internal description are shown
using editable styled text markup instead of terminal color escape sequences.
The internal description is omitted when
it is empty. This output can be copied into `@name`, `@description`, or
`@internal-description`.

Only Wizards can use `@examine`. Wizards may examine any object.

With no target, all examination modes inspect `here`. `/b` and `/d` abbreviate
`/brief` and `/debug`. Debug list pointers come from the live database; reports
never save the world or invoke Lua callbacks. Long reports are delivered in
bounded chunks to your invoking session only.

## Entrances

`@entrances [<target>][,<low>[,<high>]]` lists exits linked to the target,
room droptos and player/thing homes. The default target is your location.
Bounds are inclusive dbrefs, optionally prefixed by `#`; missing or invalid
bounds use zero and the database maximum. Results appear in ascending dbref
order with a total count. This is available only to Wizards.
