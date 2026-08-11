+++
title = "@list commands"
description = "List accessible built-in and Lua commands"
keywords = ["@list commands", "@list", "list commands"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @list commands

List commands available to you:

```text
@list commands
```

The output has separate sections for built-in commands, global Lua commands,
and object Lua commands. Lua entries show their command pattern and source.

Global commands are filtered by their configured `public`, `wizard`, or `god`
access level. Object commands are also limited to sources that can receive
commands from your current location, including your inventory, the current
room and its contents, and applicable command zones. Halted objects and
objects blocked from the relevant command scope are omitted.

Patterns are listed without being evaluated, and handlers are not run.
