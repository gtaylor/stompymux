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
and object commands, including object-local native registrations. Lua entries show their command pattern and source.

Entries are filtered by current `everyone`, `wizard` or `god` permissions.
Object entries use the current dispatcher scope: the caller, immediate location
nearby occupants, directly carried objects and zone fallbacks. Garbage, GOING,
HALTED and NO_COMMAND sources are omitted. Each entry identifies its dispatch
stage and source; zone entries are conditional fallbacks. Repeated Lua declarations retain their
module and declaration identity.

`@list permissions` shows roles and execution restrictions such as session
requirements and macro exclusions. `@list switches` shows native switch names,
permissions and minimum abbreviation lengths. Valid combinations are still
checked by each command. Lua patterns are not interpreted as switch declarations.

`@list flags` and `@list powers` retain their catalogs. `@list globals` shows
runtime cleaning, idlechecking, queueing and login status. Recognized list topics for
other deferred subsystems report that they are not implemented.

Reports are private to your connection, automatically chunked and bounded. If the
aggregate output budget is exceeded, omissions are explicitly marked. Patterns
are displayed literally; no handlers run and no database writes occur. Successful
Lua reload updates captured registrations; runtime table edits do not.

`@list site_information` (`@list si`) shows IPv4 site access and suspicion rules
in first-match order. Both lists default to unrestricted/trusted when no rule matches.
