+++
title = "Channels"
description = "Join channels, talk, and browse recent messages"
keywords = ["channels", "comsys", "addcom", "delcom", "clearcom", "comlist", "allcom"]
article_tags = ["show_in_index"]
+++

# Channels

Channels carry conversations across rooms. New players receive a `pub` alias
for the configured public channel when that channel exists.

| Command | Purpose |
| --- | --- |
| `addcom <alias>=<channel>` | Join a channel using a personal alias |
| `delcom <alias>` | Remove an alias; the last alias removes channel membership |
| `comlist` | List your aliases and listening status |
| `clearcom` | Remove all your channel aliases |
| `allcom on`, `allcom off`, `allcom who` | Apply an operation to your channels |

Aliases contain one to five printable ASCII characters with no spaces.
They are case-insensitive and take precedence over ordinary command names.
For example, `addcom pub=Public` lets you use:

- `pub Hello!` to speak.
- `pub :waves` to pose with a space after your name.
- `pub ;'s radio crackles` to pose without that space.
- `pub off` or `pub on` to stop or resume listening.
- `pub who` to see visible connected listeners and channel objects.
- `pub last` to read up to twenty recent messages, newest first.

Channel access and in-character restrictions may prevent joining or speaking.
Turning a channel off preserves membership. Reading `who` or `last` while off
requires Wizard access or the server's channel-lurking setting.

Speech is plain text; channel names and player names use the game's styled-text
renderer. A message reaches every connected session of each eligible listener.
Listening preferences, aliases and history survive restart. See [page](page.md)
for private communication and [channel administration](wizard_commands/chan.md)
for Wizard controls.
