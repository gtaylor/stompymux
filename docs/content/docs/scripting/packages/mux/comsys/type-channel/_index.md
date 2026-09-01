---
title: Channel
type: docs
sidebar_root_for: self
---

A `Channel` is a typed handle to one live communication channel. Obtain handles
with `mux.comsys.channel`, `create_channel`, or `list_channels`. Channel state is
available only through methods; assigning fields raises `mux.arg.invalid`.

| Method | Result or behavior |
| --- | --- |
| `name()` | Returns the channel name. |
| `object()` | Returns the attached Object, or `nil`. |
| `user_count()` | Returns the number of membership records. |
| `max_user_count()` | Returns the allocated membership capacity. |
| `message_count()` | Returns the lifetime message count. |
| `set_object(object)` | Attaches an Object supplying description and channel locks; `nil` detaches it. |
| `flags()` | Returns a [ChannelFlags](../type-channel-flags/) handle. |
| `emit(message, options?)` | Delivers an administrative message; `{ no_header = true }` omits `[channel]`. |
| `who(options?)` | Returns active member records; `{ all = true }` includes inactive records. |
| `boot_player(object)` | Announces a God-administered boot and removes the member's aliases. |

`who` returns a dense array of records shaped as
`{ object = Object, listening = boolean }`. The default applies the native
active-member test; `all` corresponds to `@chan/who <name>/all`. Trusted Lua
does not apply actor-relative hidden-member filtering.

`emit` validates UTF-8 and rejects embedded NUL bytes. It uses native delivery,
history, receive-lock, in-character-location, and message-count behavior.
`boot_player` accepts an Object or dbref, requires an existing membership, and
preserves the native departure and alias-removal side effects.
