+++
title = "Paging"
description = "Send private messages to connected players"
keywords = ["page", "paging"]
article_tags = ["show_in_index"]
+++

# Paging

Use `page <player>=<message>` to send a private message. Player names, player
aliases and `#dbrefs` are accepted. A full player name is tried first, so names
containing spaces work. Otherwise, separate multiple recipients with spaces.

- `page Alice=Hello!` sends ordinary text.
- `page Alice Bob=:waves` sends a pose.
- `page Alice=;'s radio crackles` joins a pose directly to your name.
- `page` reports your saved recipients.
- `page Another message` sends to your saved recipients.

The last-recipient list survives restart and includes successful recipients
only. Disconnected players cannot receive pages. In-character restrictions
apply unless either endpoint is a Wizard. There is no offline inbox.

Pages reach all sessions of a recipient. Message bodies are plain text rather
than executable styled markup. For public conversation, see [channels](channels.md).
