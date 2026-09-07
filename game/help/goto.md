+++
title = "goto"
description = "Travel through a local exit"
keywords = ["goto", "go", "move", "movement"]
article_tags = ["show_in_index"]
+++

# Exit travel

Use `goto <exit>` to follow an exit from your current location. You can also
enter the exit's name or one of its aliases directly. Names are case-insensitive.

The supplied command aliases are `go`, `got`, `m`, `mo`, `mov` and `move`.
For example, `go north` and `north` use the same exit and traversal locks.

An exit must be linked and permit you to traverse it. If the name matches
multiple equally preferred exits, use a distinct exit alias. Administrators may
restrict `goto`; those restrictions also apply to bare exit travel.

This command accepts no switches, including `/quiet`. It does not teleport to
remote objects or bypass locks. Wizard-only `home` and `@teleport` are separate
commands.
