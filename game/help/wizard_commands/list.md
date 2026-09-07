+++
title = "@list commands"
description = "List accessible built-in and Lua commands"
keywords = ["@list commands", "@list", "list commands", "@list permissions", "@list switches"]
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

Entries are filtered by current `everyone`, `wizard` or `god` permissions.
Object entries use the current dispatcher scope: the caller, immediate location
and nearby objects. HALTED and NO_COMMAND sources are omitted. Inventory and
command-zone expansion remain deferred. Repeated Lua declarations retain their
module and declaration identity.

`@list permissions` shows roles and execution restrictions such as session
requirements and macro exclusions. `@list switches` shows native switch names,
permissions and minimum abbreviation lengths. Valid combinations are still
checked by each command. Lua patterns are not interpreted as switch declarations.

`@list flags` and `@list powers` retain their catalogs. Recognized list topics for
other deferred subsystems report that they are not implemented.

Reports are private to your connection, automatically chunked and bounded. If the
aggregate output budget is exceeded, omissions are explicitly marked. Patterns
are displayed literally; no handlers run and no database writes occur. Successful
Lua reload updates captured registrations; runtime table edits do not.
