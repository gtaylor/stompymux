+++
title = "@chan/oflags"
description = "Set channel capabilities granted to objects"
keywords = ["@chan/oflags"]
article_tags = ["chan_switches"]
weight = 70
wizard_only = true
+++

# @chan/oflags

Grant or clear a capability for objects:

```text
@chan/oflags <channel>=join
@chan/oflags <channel>=receive
@chan/oflags <channel>=transmit
@chan/oflags <channel>=!<capability>
```

An enabled capability grants it to every object. Clearing a capability allows
the corresponding lock on the channel object to decide access instead.
