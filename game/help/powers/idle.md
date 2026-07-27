+++
title = "IDLE"
description = "Exempt a player from the inactivity timeout"
keywords = ["idle", "idle power"]
article_tags = ["powers"]
+++

# IDLE

`IDLE` exempts a connected player from being disconnected when the player's
inactivity timeout expires. It has no native effect on other object types.

Wizards and God receive this exemption automatically and do not need the stored
power. `IDLE` does not mark a connection active and does not exempt an
unauthenticated connection from the login timeout.

Only Wizards and God may change powers:

```text
@power <player>=idle
@power <player>=!idle
```

The first command grants the power and the second removes it. The normal
control rules apply to the target.
