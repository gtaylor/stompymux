---
title: ChannelFlags
type: docs
toc_hide: false
weight: 30
no_list: true
sidebar_root_for: self
---

Create a `ChannelFlags` handle with [`Channel:flags`](../type-channel/flags/).
Methods accept only typed constants from [`mux.comsys.flags`](../flags/).

## Methods

| Method | Description |
| --- | --- |
| [`ChannelFlags:list`](list/) | Lists the flags currently set. |
| [`ChannelFlags:has`](has/) | Tests a flag's presence. |
| [`ChannelFlags:add`](add/) | Sets a flag and reports whether it changed. |
| [`ChannelFlags:remove`](remove/) | Clears a flag and reports whether it changed. |

Flag changes run immediately and are not part of the Lua State transaction.
The collection becomes stale with its originating Channel and then raises
`mux.channel.invalid`.

## See Also

- [`mux.comsys`](../../)
- [`Channel`](../type-channel/)
- [`mux.comsys.flags`](../flags/)
