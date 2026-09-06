+++
title = "@chan/object"
description = "Attach an object to a channel"
keywords = ["@chan/object"]
article_tags = ["chan_switches"]
weight = 65
wizard_only = true
+++

# @chan/object

Attach an object to a channel, or use an empty value to detach it:

```text
@chan/object <channel>=<object>
@chan/object <channel>=
```

The attached object's description is used by `/list` and `/status`. Its
`channel_join`, `channel_transmit`, and `channel_receive` locks grant the
corresponding access.
