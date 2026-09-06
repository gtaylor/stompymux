+++
title = "Player macros"
keywords = ["macros", "macro", ".create", ".add", ".del", ".chslot", ".list", ".glist", ".ex", ".gex", ".name", ".chmod", ".chown", ".clear", ".def", ".undef"]
article_tags = ["show_in_index"]
description = "Define and share short aliases for native and Lua commands"
+++

# Player macros

Macros turn a short dot command into a longer game command. Each player has
five attachment slots, numbered **0–4**, and a selected slot for editing.
Your attachments and definitions persist across logout and server restarts.
All sessions for your player share them.

```text
.create My shortcuts
.def hi=say Hello, *!
.hi everyone
.def who=WHO
.who
```

Aliases contain 1–4 printable ASCII characters, without whitespace. Definitions,
lookup and removal are case-insensitive. An existing alias must be removed with
`.undef` before it can be redefined. The expansion can contain Unicode.

Every `*` inserts the arguments after the alias; `%*` inserts a literal asterisk.
Arguments are not automatically appended. Expansion happens once: it cannot invoke
another macro or a macro-management command. Semicolons are not command separators.
An expansion can invoke native commands, Lua commands, exits or channel aliases,
with your normal permissions. Oversized expansions fail without executing.

## Attachments and definitions

| Command | Meaning |
| --- | --- |
| `.create <description>` | Create a private, unlocked set in the first empty slot and select it |
| `.add <set number>` | Attach a readable shared set in the first empty slot; keep the editing selection |
| `.del <slot>` | Detach a slot without destroying the shared set |
| `.chslot <slot>` | Select an attached slot for editing |
| `.list` | List your five slots and the current editing slot |
| `.glist` | List global sets you can read |
| `.ex [slot]` | Examine an attached set, defaulting to your editing slot |
| `.gex <set number>` | Examine a readable global set |
| `.def <alias>=<command>` | Define an alias in the selected set |
| `.undef <alias>` | Remove an alias from the selected set |
| `.name <description>` | Rename the selected set if you can write it |
| `.clear` | Destroy the selected set and remove every attachment to it |

Invocation checks slots from **0 through 4**; the first matching alias wins.
Selecting an editing slot does not change this order. Attaching the same set
more than once is allowed. Management command names are reserved.

Set numbers are global and contiguous. Destroying a set renumbers later sets
and updates everyone's attachments automatically. Slot numbers do not move.

## Sharing and permissions

Use `.chmod L`, `.chmod R` or `.chmod W` on the selected set. Prefix a mode
with `!` to clear it, for example `.chmod !L`.

- **L** locks the set against definition edits and renaming, including by its owner.
- **R** lets other players discover, inspect and attach the set.
- **W** lets other players edit the set while it is unlocked.

Only the owner or a Wizard may change modes. Wizards may read any set, but
must still satisfy its write policy to edit definitions or rename it. Wizard-only
`.chown <object>` changes the selected set's owner.

Removing **R** does not revoke existing attachments: those players can still
invoke and locally inspect the set. To stop an alias, remove its definition.

Owners may `.clear` their own unlocked sets. Wizards may also clear another
owner's set even when locked; a Wizard's own locked set must first be unlocked.
Changes become visible only after they save successfully. Inspection and
confirmation messages go only to your invoking session.
