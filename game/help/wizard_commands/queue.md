+++
title = "Command queues"
keywords = ["command queues", "@force", "@wait", "@halt"]
article_tags = ["wizard_commands"]
description = "Force commands, delay execution, and cancel queued work"
wizard_only = true
+++

# Command queues

These commands require Wizard access. Configured command aliases work normally.

| Command | Effect |
| --- | --- |
| `@force <object>=<commands>` | Queue commands for a controlled object |
| `@wait <seconds>=<commands>` | Queue your commands after an integer delay |
| `@halt` | Cancel your ready and delayed command lists |
| `@halt <object>` | Cancel that object's command lists |
| `@halt/all` | Cancel every object's ready and delayed lists |

For example:

```text
@force QueueRobot=say Hello.;pose waves.
@wait 5={say Five seconds have passed.;look}
@halt QueueRobot
```

Nonpositive waits become ready immediately. Semicolons separate commands unless
protected by braces, brackets, parentheses or a backslash. Escapes remain literal;
there is no expression evaluation. `@wait` accepts an outer pair of braces around
the command list. A queued list runs in order, yielding between commands so
other objects and connections can make progress.

Forced commands use the target's permissions. Forcing a non-Wizard to run a
Wizard command does not grant them permission. Wizards cannot force other
Wizards; GOD can. `@force` and `@wait` cannot themselves be invoked through
player macros. Queued commands can use the executing player's macros.

Background execution has no connection. Connection-specific commands such as
`quit`, `color`, `help`, `@find`, `@session`, `@telnet`, and interactive
administration tooling report that an interactive session is required.
World commands and Lua commands still work, including for disconnected players.
Replies use ordinary object notification; player output reaches all their
connected sessions.

Queue limits apply equally to players and things. Overflow cancels that object's
pending work and sets HALTED. `@halt` cancels work without setting HALTED; use
`@flag <object>=!halted` to clear an overflow halt. Destroyed or HALTED executors
do not run. `/all` cannot be combined with a target.

Command lists are not saved. Disconnecting does not cancel them, but server
shutdown discards all pending work. A failing command rolls back its changes
and output; later commands can still run. No `#<dbref> command` shorthand exists.
