---
title: flags
type: docs
---

`mux.comsys.flags` contains immutable typed constants accepted by
[`ChannelFlags`](type-channel-flags/):

| Constant | Native behavior |
| --- | --- |
| `PUBLIC` | Makes the channel visible without a successful join lock. |
| `LOUD` | Announces applicable connection and presence changes. |
| `TRANSPARENT` | Relaxes hidden-member filtering in native channel displays. |

Constants compare by identity within the current Lua runtime and render as
their uppercase names. Unknown constants, raw strings, and constants from
another runtime raise `mux.channel_flag.invalid`.
