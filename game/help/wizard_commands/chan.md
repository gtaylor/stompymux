+++
title = "Channel administration"
description = "Create, configure and inspect communication channels"
keywords = ["@chan", "chan"]
article_tags = ["wizard_commands"]
wizard_only = true
show_index_for_article_tags = ["chan_switches"]
index_style = "list_with_description"
+++

# Channel administration

`@chan` lists its switches. All forms require Wizard access.

| Command | Purpose |
| --- | --- |
| `@chan/create <channel>` | Create a private channel |
| `@chan/destroy <channel>` | Remove it, its aliases and its history |
| `@chan/list` | List channels and descriptions |
| `@chan/list/full` | Include permissions, attached objects and counters |
| `@chan/status <channel>` | Inspect a single channel |
| `@chan/status/full <channel>` | Inspect detailed channel metadata |
| `@chan/who <channel>[/all]` | List current members, optionally including offline players |
| `@chan/object <channel>=<object>` | Attach the object providing description and locks |
| `@chan/object <channel>=#-1` | Detach the channel object |
| `@chan/flags <channel>=<flag>` | Set `public`, `loud` or `transparent` |
| `@chan/pflags <channel>=<permission>` | Set player `join`, `transmit` or `receive` permission |
| `@chan/oflags <channel>=<permission>` | Set the corresponding object permission |
| `@chan/boot <channel>=<object>` | Remove a member; you must be a member yourself |
| `@chan/emit <channel>=<message>` | Emit styled text with a channel header |
| `@chan/emit/noheader <channel>=<message>` | Emit styled text without a header |

Prefix a flag or permission with `!` to clear it. Channel names contain at most
49 printable ASCII characters, with no spaces. New channels grant the player
and object access bits but are not PUBLIC.

`LOUD` announces first connections and final disconnections. `TRANSPARENT`
preserves the fork's visibility policy; it does not reveal DARK players to
ordinary viewers. Channel locks can grant access independently of access bits;
a false or failed lock does not revoke an explicit bit grant. Wizards bypass
channel access checks.

Durable mutations commit before message delivery. Unrelated macro records and
unknown database columns remain untouched. See [player macros](../macros.md) for persistent dot-command shortcuts.
