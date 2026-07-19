+++
title = "@chan/emit"
description = "Send an administrative message to a channel"
keywords = ["@chan/emit"]
article_tags = ["chan_switches"]
weight = 50
wizard_only = true
+++

# @chan/emit

Send text directly to every active listener:

```text
@chan/emit <channel>=<message>
@chan/emit/noheader <channel>=<message>
```

The normal form adds the channel name in brackets. `/noheader` sends exactly
the supplied text.
