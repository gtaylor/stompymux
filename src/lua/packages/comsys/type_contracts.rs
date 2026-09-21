//! LuaLS contract blocks for the mux comsys surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00059
//|---Adds a player to this channel with a player-local command alias. The trusted
//|---operation bypasses the channel join lock. A quiet join suppresses only the
//|---channel-wide announcement; direct confirmations are still sent to the player.
//|---@param player DbRef|Object Player to add.
//|---@param alias string One to five printable ASCII characters without spaces.
//|---@param quiet boolean Whether to suppress the channel-wide join announcement.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.internal`](lua://mux.error.codes.internal).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.internal
//|function Channel:add_player(player, alias, quiet) end
// lua-types-end

// lua-types-begin mux 00060
//|---Announces a God-administered boot and removes a current member's channel
//|---aliases using the native side-effect path.
//|---@param object DbRef|Object Current channel member.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.object.invalid
//|function Channel:boot_player(object) end
// lua-types-end

// lua-types-begin mux 00061
//|---Emits an administrative channel message through native delivery, history,
//|---receive-lock, and message-count behavior.
//|---@param message string Valid UTF-8 without embedded NUL bytes.
//|---@param options? ChannelEmitOptions Unknown option fields are rejected.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.arg.invalid
//|function Channel:emit(message, options) end
// lua-types-end

// lua-types-begin mux 00062
//|---Opens the live administrative flag collection for this channel.
//|---@return ChannelFlags flags
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|function Channel:flags() end
// lua-types-end

// lua-types-begin mux 00063
//|---Returns the channel's currently allocated membership capacity.
//|---@return integer count
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|function Channel:max_user_count() end
// lua-types-end

// lua-types-begin mux 00064
//|---Returns the channel's lifetime delivered-message count.
//|---@return integer count
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|function Channel:message_count() end
// lua-types-end

// lua-types-begin mux 00065
//|---Returns the channel's exact name.
//|---@return string name
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|function Channel:name() end
// lua-types-end

// lua-types-begin mux 00066
//|---Returns the object that supplies the channel description and locks.
//|---@return Object? object The attached object, or nil when none is attached.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.object.invalid
//|function Channel:object() end
// lua-types-end

// lua-types-begin mux 00067
//|---Attaches an object that supplies channel locks and description, or detaches
//|---the current object when passed nil.
//|---@param object DbRef|Object|nil
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `object` is omitted, [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function Channel:set_object(object) end
// lua-types-end

// lua-types-begin mux 00068
//|---Returns the number of channel membership records.
//|---@return integer count
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|function Channel:user_count() end
// lua-types-end

// lua-types-begin mux 00069
//|---Returns channel membership records. By default the native active-member
//|---filter is applied; `options.all` includes inactive records.
//|---@param options? ChannelWhoOptions Unknown option fields are rejected.
//|---@return ChannelMember[] members
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.arg.invalid
//|function Channel:who(options) end
// lua-types-end

// lua-types-begin mux 00070
//|---Sets a typed channel flag.
//|---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
//|---@return boolean changed Whether the flag changed from unset to set.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.channel_flag.invalid
//|function ChannelFlags:add(flag) end
// lua-types-end

// lua-types-begin mux 00071
//|---Tests whether the channel has a typed flag.
//|---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
//|---@return boolean present
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.channel_flag.invalid
//|function ChannelFlags:has(flag) end
// lua-types-end

// lua-types-begin mux 00072
//|---Lists set flags in `PUBLIC`, `LOUD`, `TRANSPARENT` order.
//|---@return ChannelFlag[] flags
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|function ChannelFlags:list() end
// lua-types-end

// lua-types-begin mux 00073
//|---Clears a typed channel flag.
//|---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
//|---@return boolean changed Whether the flag changed from set to unset.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|---@see mux.error.codes.channel_flag.invalid
//|function ChannelFlags:remove(flag) end
// lua-types-end

// lua-types-begin mux 00116
//|---Retrieves an existing communication channel by case-insensitive name.
//|---@param name string Existing channel name without embedded NUL bytes; the returned handle preserves canonical spelling.
//|---@return Channel channel
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.channel.invalid
//|function mux_comsys.channel(name) end
// lua-types-end

// lua-types-begin mux 00117
//|---Creates a private communication channel using the native channel-name
//|---rules. Names must be non-empty printable ASCII, contain no spaces, and be
//|---shorter than 50 bytes.
//|---@param name string New channel name.
//|---@return Channel channel
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid) when the name already exists.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.channel.invalid
//|function mux_comsys.create_channel(name) end
// lua-types-end

// lua-types-begin mux 00118
//|---Permanently removes a live channel and its membership storage. The supplied
//|---handle and every flag handle derived from it become stale.
//|---@param channel Channel Live channel handle.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.channel.invalid
//|function mux_comsys.destroy_channel(channel) end
// lua-types-end

// lua-types-begin mux 00119
//|---Lists every live communication channel in case-insensitive name order, with
//|---original spelling used as the tie-breaker.
//|---@return Channel[] channels
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
//|---or [`mux.error.codes.internal`](lua://mux.error.codes.internal) if the native
//|---registry count changes while it is copied.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.internal
//|function mux_comsys.list_channels() end
// lua-types-end
