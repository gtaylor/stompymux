---
title: mux.comsys
linkTitle: mux.comsys
type: docs
weight: 14
no_list: true
sidebar_root_for: self
---

`mux.comsys` is the trusted communication-channel API. It operates directly on
the live channel registry as God; changes take effect immediately and are not
rolled back if the Lua callback later fails. Existing `@chan/*` commands retain
their normal Wizard checks and player-facing output.

All functions are unavailable during `@lua/check`. Unknown or stale channels
raise `mux.channel.invalid`. A Channel handle stays stale after destruction even
if another channel is created with the same name.

## Namespaces

| Type | Description |
| --- | --- |
| [`Channel`](type-channel/) | A validated handle for a live communication channel. |
| [`ChannelFlags`](type-channel-flags/) | Reads and changes a channel's administrative flags. |

## Functions

| Function | Description |
| --- | --- |
| [`channel`](channel/) | Retrieves an existing channel by name. |
| [`create_channel`](create-channel/) | Creates a private channel. |
| [`destroy_channel`](destroy-channel/) | Permanently removes a channel. |
| [`list_channels`](list-channels/) | Lists channels in deterministic name order. |

## Constants

| Constants package | Description |
| --- | --- |
| [`flags`](flags/) | Typed constants for native channel flags. |
