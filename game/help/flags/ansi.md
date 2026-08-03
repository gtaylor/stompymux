+++
title = "ANSI"
description = "Enable ANSI color and formatting"
keywords = ["ansi"]
article_tags = ["flags"]
+++

# ANSI (`X`)

Enables ANSI color and formatting, including BattleTech map displays, for a
player. Without this flag, ANSI escape sequences and object color markup are
rendered as plain text. When enabled,
the server uses MTTS terminal negotiation to select truecolor, 256-color, or
16-color output for each connection. Clients reporting screen-reader mode
receive plain text.
