+++
title = "@list commands"
description = "List accessible built-in and Lua commands"
keywords = ["@list commands", "@list", "list commands", "@list permissions", "@list switches", "@list config_permissions", "@list options", "@list default_flags", "@list bad_names", "@list process"]
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

Entries are filtered by effective configured permissions. Dark and disabled commands
are omitted. Wizard and GOD role bits are alternatives; `!wizard god` makes an
existing Wizard command GOD-only.
Object entries use the current dispatcher scope: the caller, immediate location
nearby occupants, directly carried objects and zone fallbacks. Garbage, GOING,
HALTED and NO_COMMAND sources are omitted. Each entry identifies its dispatch
stage and source; zone entries are conditional fallbacks. Repeated Lua declarations retain their
module and declaration identity.

`@list permissions` shows effective access bits and execution restrictions such as session
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

The `@list` command and the requested topic must both permit you. Configuration
can relax the command's default Wizard restriction while retaining private topics.
Policies use ordered set/clear edits (`wizard`, `!wizard`, `disabled`, `dark`, and
other supported access bits). They apply to native/Lua registrations and native
switches at startup and Lua reload. Unknown targets reject a reload, preserving
the active runtime. GOD may inspect disabled switch names; those switches remain
unavailable for execution.


`@list config_permissions` shows directive access and whether each directive is
live, restart-only or unsupported. `@list options` shows implemented runtime
settings, `@list default_flags` shows new-object defaults, and `@list bad_names`
lists disallowed name patterns. See [runtime administration](admin.md).

## Process statistics

`@list process` (abbreviated `@list pr`) reports process-wide host resource usage.
It requires Wizard access by default and supports queued execution. CPU time is
in seconds; page size and peak resident memory are bytes. Descriptor values are
soft/hard limits, not free slots. Block I/O counts operations, and IPC counts
messages rather than socket bytes. Unsupported counters say `unavailable` instead
of misleadingly reporting zero. Collection is on demand with a bounded deadline.
