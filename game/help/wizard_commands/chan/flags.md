+++
title = "@chan/flags"
description = "Set or clear channel flags"
keywords = ["@chan/flags"]
article_tags = ["chan_switches"]
weight = 90
wizard_only = true
+++

# @chan/flags

Enable a channel flag, or prefix it with `!` to clear it:

```text
@chan/flags <channel>=public
@chan/flags <channel>=!public
@chan/flags <channel>=loud
@chan/flags <channel>=!loud
@chan/flags <channel>=transparent
@chan/flags <channel>=!transparent
```

Public channels appear in public listings. Loud channels announce connects and
disconnects. Transparent channels reveal eligible hidden listeners in channel
membership output.

See `help @chan/object` to attach an object that supplies the channel's
description and access locks.
