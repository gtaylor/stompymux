---
title: Channel
type: docs
toc_hide: false
weight: 20
no_list: true
sidebar_root_for: self
---

A `Channel` is a typed handle to one live communication channel. Obtain handles
with [`mux.comsys.channel`](../channel/),
[`mux.comsys.create_channel`](../create-channel/), or
[`mux.comsys.list_channels`](../list-channels/). Channel state is available only
through methods; assigning fields raises `mux.arg.invalid`.

## Methods

| Method | Description |
| --- | --- |
| [`Channel:name`](name/) | Returns the channel name. |
| [`Channel:object`](object/) | Returns the attached Object, or `nil`. |
| [`Channel:user_count`](user-count/) | Returns the number of membership records. |
| [`Channel:max_user_count`](max-user-count/) | Returns the allocated membership capacity. |
| [`Channel:message_count`](message-count/) | Returns the lifetime message count. |
| [`Channel:set_object`](set-object/) | Attaches an Object supplying description and channel locks; `nil` detaches it. |
| [`Channel:flags`](flags/) | Returns a [`ChannelFlags`](../type-channel-flags/) handle. |
| [`Channel:emit`](emit/) | Delivers an administrative message; `{ no_header = true }` omits `[channel]`. |
| [`Channel:who`](who/) | Returns active member records; `{ all = true }` includes inactive records. |
| [`Channel:add_player`](add-player/) | Adds a player with an alias and controls whether the join is announced. |
| [`Channel:boot_player`](boot-player/) | Announces a God-administered boot and removes the member's aliases. |

`who` returns a dense array of records shaped as
`{ object = Object, listening = boolean }`. The default applies the native
active-member test; `all` corresponds to `@chan/who <name>/all`. Trusted Lua
does not apply actor-relative hidden-member filtering.

`emit` validates UTF-8 and rejects embedded NUL bytes. It uses native delivery,
history, receive-lock, in-character-location, and message-count behavior.
`add_player` accepts an Object or dbref for a player, registers a player-local
alias, and bypasses the channel join lock. A quiet join still sends the player
the normal direct confirmations but omits the channel-wide announcement.
`boot_player` accepts an Object or dbref, requires an existing membership, and
preserves the native departure and alias-removal side effects.

## See Also

- [`mux`](../../)
- [`mux.comsys`](../)
- [`Object`](../../world/type-object/)
