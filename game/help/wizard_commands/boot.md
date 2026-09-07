+++
title = "@boot"
keywords = ["@boot", "@boot/port", "@boot/quiet"]
article_tags = ["wizard_commands"]
description = "Disconnect player sessions"
wizard_only = true
+++

# @boot

```text
@boot <player>
@boot/quiet <player>
@boot/port <session ID>
@boot/port/quiet <session ID>
```

A player target closes every session for that player. GOD and the caller cannot
be booted through this form. Player names, aliases and dbrefs are accepted.

`/port` selects a stable Rust session ID, as displayed by `@session`; it does not
select a listener port or operating-system descriptor. It can close an
unauthenticated connection or the caller's own session. Only GOD can close a GOD
session this way. `/p` and `/q` abbreviate the switches.

Victims normally receive a departure message; `/quiet` suppresses it. The caller
receives the number of connections selected for closure. Disconnect hooks and
CONNECTED tracking use the ordinary disconnect lifecycle.
