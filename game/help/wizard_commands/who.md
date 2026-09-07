+++
title = "@who"
description = "Inspect authenticated player connections"
keywords = ["@who", "administrative who"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @who

`@who [prefix]` lists all authenticated sessions, including hidden players and
multiple connections for one player. Prefixes match player names without styling,
case-insensitively. An active connection is required; switches are not supported.

Columns show player name, connected and idle times, `D` (DARK) and `+` (SUSPECT)
markers, immediate location, accepted command count and peer IP. The footer shows
matched sessions, the record count and configured maximum.

Denied and unknown attempts count. Login input, interactive-flow responses,
queued commands and quota-rejected input do not. Exact `IDLE` (any letter case)
outside a flow is a silent keepalive that consumes quota but does not reset idle
time or increment the count.

Use public `WHO` for the Lua player listing, `@session` for connection byte/queue
statistics, and `@telnet <player>` for negotiated protocol details.
