+++
title = "@chan/pflags"
description = "Set channel capabilities granted to players"
keywords = ["@chan/pflags"]
article_tags = ["chan_switches"]
weight = 80
wizard_only = true
+++

# @chan/pflags

Grant or clear a capability for players:

```text
@chan/pflags <channel>=join
@chan/pflags <channel>=receive
@chan/pflags <channel>=transmit
@chan/pflags <channel>=!<capability>
```

An enabled capability grants it to every player. Clearing a capability allows
the corresponding lock on the channel object to decide access instead.
