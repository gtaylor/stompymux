//! LuaLS contract blocks for the mux root surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00000
//|---@alias ConfigValue string|number|boolean Scalar value returned by the live configuration registry.
// lua-types-end

// lua-types-begin mux 00001
//|---@alias DbRef integer Database object reference.
// lua-types-end

// lua-types-begin mux 00002
//|---@alias NativeErrorRoot "mux"|"btech"|"testing" Root of a checked native error-code tree.
// lua-types-end

// lua-types-begin mux 00003
//|---@alias StateValue string|boolean|number Scalar value supported by persistent object state.
// lua-types-end

// lua-types-begin mux 00004
//|---@alias TelnetEnvironmentKind "var"|"uservar" RFC 1572 NEW-ENVIRON variable namespace.
// lua-types-end

// lua-types-begin mux 00005
//|---Exact fields accepted by [`mux.world.create_object`](lua://mux.world.create_object).
//|---The selected typed object-kind constant determines which other fields apply.
//|---@alias CreateObjectOptions CreateRoomOptions|CreateThingOptions|CreateExitOptions
// lua-types-end

// lua-types-begin mux 00006
//|---Options for [`Channel:emit`](lua://Channel.emit).
//|---@class (exact) ChannelEmitOptions
//|---@field no_header? boolean Send the message without the usual `[channel]` prefix.
// lua-types-end

// lua-types-begin mux 00007
//|---Options for [`Channel:who`](lua://Channel.who).
//|---@class (exact) ChannelWhoOptions
//|---@field all? boolean Include inactive membership records.
// lua-types-end

// lua-types-begin mux 00008
//|---One communication-channel membership record.
//|---@class ChannelMember
//|---@field object Object Live member object.
//|---@field listening boolean Whether the member is currently listening to the channel.
// lua-types-end

// lua-types-begin mux 00009
//|---A generation-sensitive handle to one live communication channel. Equality
//|---requires the same package, native channel identity, and generation. Handles
//|---remain stale after destruction even if a channel with the same name is
//|---created later. Assigning fields raises
//|---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
//|---@see mux.error.codes.arg.invalid
//|---@class Channel
//|local Channel = {}
// lua-types-end

// lua-types-begin mux 00010
//|---A typed communication-channel flag constant from
//|---[`mux.comsys.flags`](lua://mux.comsys.flags). Its string form is the
//|---canonical uppercase name, and equality compares identity within the current
//|---Lua runtime.
//|---@class ChannelFlag
// lua-types-end

// lua-types-begin mux 00011
//|---A live view of one channel's administrative flags. It becomes stale when
//|---its originating channel is destroyed. `tostring` returns
//|---`channel_flags(<name>)` and can raise
//|---[`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
//|---or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
//|---@class ChannelFlags
//|local ChannelFlags = {}
// lua-types-end

// lua-types-begin mux 00012
//|---A checked flag constant obtained from [`mux.world.flags`](lua://mux.world.flags).
//|---Its string form is the canonical uppercase native name, and equality compares
//|---the native flag identity within the current runtime.
//|---String conversion raises [`mux.error.codes.internal`](lua://mux.error.codes.internal) if a registered native name exceeds the internal conversion buffer.
//|---@class Flag
// lua-types-end

// lua-types-begin mux 00013
//|---A checked power constant obtained from [`mux.world.powers`](lua://mux.world.powers).
//|---Its string form is the canonical uppercase native name, and equality compares
//|---the native power identity within the current runtime.
//|---String conversion raises [`mux.error.codes.internal`](lua://mux.error.codes.internal) if a registered native name exceeds the internal conversion buffer.
//|---@class Power
// lua-types-end

// lua-types-begin mux 00014
//|---A checked command-access constant obtained from [`mux.world.access`](lua://mux.world.access).
//|---Its string form is its uppercase name, and equality compares access identity.
//|---@class Access
// lua-types-end

// lua-types-begin mux 00015
//|---The minimum shape returned when [`mux.error.pcall`](lua://mux.error.pcall)
//|---catches a table carrying a string error code. Such a table is not guaranteed
//|---to use the native [`Error`](lua://Error) metatable.
//|---@class CaughtError
//|---@field code string Stable dotted error code.
//|---@field traceback string Traceback captured by `mux.error.pcall`.
//|---@field message? string Optional human-readable failure description.
//|---@field detail? any Optional structured context.
//|---@field cause? any Optional earlier failure.
// lua-types-end

// lua-types-begin mux 00016
//|---A checked error-code symbol. Calling `tostring` returns its dotted `code`.
//|---@class ErrorCode
//|---@field code string Fully qualified error code represented by this node.
// lua-types-end

// lua-types-begin mux 00017
//|---@class ErrorFields
//|---@field code string|ErrorCode Stable dotted code or checked code node.
//|---@field message string Human-readable failure description.
//|---@field detail? any Optional structured context.
//|---@field cause? any Optional earlier failure.
// lua-types-end

// lua-types-begin mux 00018
//|---@class ErrorCodeTree: ErrorCode
//|---@field [string] ErrorCodeTree Checked child code segment.
// lua-types-end

// lua-types-begin mux 00019
//|---A structured Lua failure raised by native and script APIs. Calling `tostring`
//|---renders its stable code followed by its human-readable message.
//|---@class Error
//|---@field code string Stable dotted error code.
//|---@field message string Human-readable failure description.
//|---@field detail? any Optional structured context.
//|---@field cause? any Earlier failure preserved by wrapping.
//|---@field traceback? string Traceback added by [`mux.error.pcall`](lua://mux.error.pcall).
//|local Error = {}
// lua-types-end

// lua-types-begin mux 00020
//|---A generation-checked view of the flags set on one object. `tostring`
//|---returns `flags(#<dbref>)`; a stale object raises
//|---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@class Flags
//|local Flags = {}
// lua-types-end

// lua-types-begin mux 00021
//|---A typed native lock obtained from [`mux.world.locks`](lua://mux.world.locks).
//|---Its string form is its uppercase name, and equality compares lock identity
//|---within the current runtime.
//|---@class Lock
// lua-types-end

// lua-types-begin mux 00022
//|---A generation-checked native database object handle. Native equality compares
//|---object identity. `tostring` returns `object(#<dbref>)` and raises
//|---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
//|---when the handle is stale.
//|---@class Object
//|local Object = {}
// lua-types-end

// lua-types-begin mux 00023
//|---A typed native object kind obtained from [`mux.world.types`](lua://mux.world.types).
//|---Its string form is its uppercase name, and equality compares native type
//|---identity within the current runtime.
//|---@class ObjectType
// lua-types-end

// lua-types-begin mux 00024
//|---The `ROOM` object-kind constant from the current runtime.
//|---@class RoomObjectType: ObjectType
// lua-types-end

// lua-types-begin mux 00025
//|---The `THING` object-kind constant from the current runtime.
//|---@class ThingObjectType: ObjectType
// lua-types-end

// lua-types-begin mux 00026
//|---The `EXIT` object-kind constant from the current runtime.
//|---@class ExitObjectType: ObjectType
// lua-types-end

// lua-types-begin mux 00027
//|---The `PLAYER` object-kind constant from the current runtime.
//|---@class PlayerObjectType: ObjectType
// lua-types-end

// lua-types-begin mux 00028
//|---A generation-checked view of the powers granted to one object. `tostring`
//|---returns `powers(#<dbref>)`; a stale object raises
//|---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@class Powers
//|local Powers = {}
// lua-types-end

// lua-types-begin mux 00029
//|---One player connection visible to the ordinary WHO command.
//|---@class Connection
//|---@field object Object Connected player.
//|---@field name string Current object name.
//|---@field connected_for integer Connected duration in seconds.
//|---@field idle_for integer Idle duration in seconds.
// lua-types-end

// lua-types-begin mux 00030
//|---Non-privileged server population statistics.
//|---@class WhoSummary
//|---@field hidden integer Hidden-player count; currently always zero for this non-privileged view.
//|---@field record integer Record simultaneous-player count.
//|---@field maximum? integer Configured limit, or nil when unlimited.
// lua-types-end

// lua-types-begin mux 00031
//|---Persistent state entry returned by [`State:entries`](lua://State.entries).
//|---@class StateEntry
//|---@field key string Stored state key.
//|---@field value StateValue Stored scalar value.
// lua-types-end

// lua-types-begin mux 00032
//|---A persistent, object-scoped state namespace. `tostring` returns
//|---`state(#<dbref>, <namespace>)` and raises
//|---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
//|---when the underlying object is stale.
//|---@class State
//|local State = {}
// lua-types-end

// lua-types-begin mux 00033
//|---Optional styled-text attributes applied by [`mux.text.style`](lua://mux.text.style).
//|---@class StyleOptions
//|---@field foreground? string Palette foreground name.
//|---@field background? string Palette background name.
//|---@field bold? boolean Whether to enable bold intensity.
//|---@field underline? boolean Whether to underline the text.
//|---@field inverse? boolean Whether to swap foreground and background presentation.
// lua-types-end

// lua-types-begin mux 00034
//|---Fields accepted when creating a detached room through
//|---[`mux.world.create_object`](lua://mux.world.create_object).
//|---@class (exact) CreateRoomOptions
//|---@field type RoomObjectType The current runtime's [`mux.world.types.ROOM`](lua://mux.world.types.ROOM) constant.
//|---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
//|---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.
// lua-types-end

// lua-types-begin mux 00035
//|---Fields accepted when creating and placing a thing through
//|---[`mux.world.create_object`](lua://mux.world.create_object).
//|---@class (exact) CreateThingOptions
//|---@field type ThingObjectType The current runtime's [`mux.world.types.THING`](lua://mux.world.types.THING) constant.
//|---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
//|---@field location DbRef|Object Required object that can contain the new thing.
//|---@field home? DbRef|Object Home object; defaults to `location` when omitted.
//|---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.
// lua-types-end

// lua-types-begin mux 00036
//|---Fields accepted when creating and attaching an exit through
//|---[`mux.world.create_object`](lua://mux.world.create_object).
//|---@class (exact) CreateExitOptions
//|---@field type ExitObjectType The current runtime's [`mux.world.types.EXIT`](lua://mux.world.types.EXIT) constant.
//|---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
//|---@field location DbRef|Object Required source object capable of holding exits.
//|---@field destination? DbRef|Object Optional destination capable of containing objects; omission leaves the exit unlinked.
//|---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.
// lua-types-end

// lua-types-begin mux 00037
//|---Fields accepted when teleporting a thing or player.
//|---@class (exact) TeleportOptions
//|---@field object DbRef|Object Required thing or player to move.
//|---@field destination DbRef|Object Required object capable of containing objects.
// lua-types-end

// lua-types-begin mux 00038
//|---Options controlling object destruction.
//|---@class (exact) DestroyOptions
//|---@field override? boolean Whether to bypass the target's SAFE flag; core objects and Wizard players remain protected.
// lua-types-end

// lua-types-begin mux 00039
//|---Fields selecting a native lock invocation to test.
//|---@class (exact) LockPassesOptions
//|---@field object DbRef|Object Required object whose lock is tested.
//|---@field enactor DbRef|Object Required object attempting the action.
//|---@field lock Lock Required typed lock constant from [`mux.world.locks`](lua://mux.world.locks).
//|---@field cause? DbRef|Object Object that caused the action; defaults to `enactor`.
//|---@field subject? DbRef|Object Object acted upon in the lock context; defaults to `enactor`.
// lua-types-end

// lua-types-begin mux 00040
//|---Filters for [`Object:contents`](lua://Object.contents).
//|---@class (exact) ContentsOptions
//|---@field types? ObjectType[] Native object kinds to include; an empty array matches nothing.
//|---@field visible_to? DbRef|Object Viewer whose native visibility rules are applied.
// lua-types-end

// lua-types-begin mux 00041
//|---Filters for [`mux.world.list_objects`](lua://mux.world.list_objects).
//|---@class (exact) ListObjectsOptions
//|---@field types? ObjectType[] Native object kinds to include; an empty array matches nothing.
//|---@field in_zone? DbRef|Object Include only objects directly assigned to this zone.
// lua-types-end

// lua-types-begin mux 00042
//|---Immutable namespace of supported communication-channel flags. Unknown or
//|---non-string lookups and attempted mutation raise
//|---[`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
//|---@class (exact) ChannelFlagNamespace
//|---@field PUBLIC ChannelFlag Makes the channel visible without a successful join lock.
//|---@field LOUD ChannelFlag Announces applicable connection and presence changes.
//|---@field TRANSPARENT ChannelFlag Relaxes hidden-member filtering in native channel displays.
//|---@see mux.error.codes.channel_flag.invalid
// lua-types-end

// lua-types-begin mux 00043
//|---Checked `mux.arg.invalid` error-code node.
//|---@class MuxArgInvalidErrorCode: ErrorCode
//|---@field code "mux.arg.invalid"
//|---Checked `mux.unavailable.checking` error-code node.
//|---@class MuxCheckingUnavailableErrorCode: ErrorCode
//|---@field code "mux.unavailable.checking"
//|---Checked `mux.runtime` error-code node.
//|---@class MuxRuntimeErrorCode: ErrorCode
//|---@field code "mux.runtime"
//|---Checked `mux.state.invalid` error-code node.
//|---@class MuxStateInvalidErrorCode: ErrorCode
//|---@field code "mux.state.invalid"
//|---Checked `mux.state.value_too_large` error-code node.
//|---@class MuxStateValueTooLargeErrorCode: ErrorCode
//|---@field code "mux.state.value_too_large"
//|---Checked `mux.state.unavailable` error-code node.
//|---@class MuxStateUnavailableErrorCode: ErrorCode
//|---@field code "mux.state.unavailable"
//|---Checked `mux.object.invalid` error-code node.
//|---@class MuxObjectInvalidErrorCode: ErrorCode
//|---@field code "mux.object.invalid"
//|---Checked `mux.object.unavailable` error-code node.
//|---@class MuxObjectUnavailableErrorCode: ErrorCode
//|---@field code "mux.object.unavailable"
//|---Checked `mux.flag.invalid` error-code node.
//|---@class MuxFlagInvalidErrorCode: ErrorCode
//|---@field code "mux.flag.invalid"
//|---Checked `mux.power.invalid` error-code node.
//|---@class MuxPowerInvalidErrorCode: ErrorCode
//|---@field code "mux.power.invalid"
//|---Checked `mux.access.invalid` error-code node.
//|---@class MuxAccessInvalidErrorCode: ErrorCode
//|---@field code "mux.access.invalid"
//|---Checked `mux.connection.invalid` error-code node.
//|---@class MuxConnectionInvalidErrorCode: ErrorCode
//|---@field code "mux.connection.invalid"
//|---Checked `mux.connection.unavailable` error-code node.
//|---@class MuxConnectionUnavailableErrorCode: ErrorCode
//|---@field code "mux.connection.unavailable"
//|---Checked `mux.channel.invalid` error-code node.
//|---@class MuxChannelInvalidErrorCode: ErrorCode
//|---@field code "mux.channel.invalid"
//|---Checked `mux.channel_flag.invalid` error-code node.
//|---@class MuxChannelFlagInvalidErrorCode: ErrorCode
//|---@field code "mux.channel_flag.invalid"
//|---Checked `mux.text.invalid` error-code node.
//|---@class MuxTextInvalidErrorCode: ErrorCode
//|---@field code "mux.text.invalid"
//|---Checked `mux.module.invalid` error-code node.
//|---@class MuxModuleInvalidErrorCode: ErrorCode
//|---@field code "mux.module.invalid"
//|---Checked `mux.module.unavailable` error-code node.
//|---@class MuxModuleUnavailableErrorCode: ErrorCode
//|---@field code "mux.module.unavailable"
//|---Checked `mux.config.not_found` error-code node.
//|---@class MuxConfigNotFoundErrorCode: ErrorCode
//|---@field code "mux.config.not_found"
//|---Checked `mux.config.unsupported` error-code node.
//|---@class MuxConfigUnsupportedErrorCode: ErrorCode
//|---@field code "mux.config.unsupported"
//|---Checked `mux.internal` error-code node.
//|---@class MuxInternalErrorCode: ErrorCode
//|---@field code "mux.internal"
// lua-types-end

// lua-types-begin mux 00044
//|---@class MuxArgErrorCodes: ErrorCode
//|---@field invalid MuxArgInvalidErrorCode `mux.arg.invalid`.
//|---@class MuxUnavailableErrorCodes: ErrorCode
//|---@field checking MuxCheckingUnavailableErrorCode `mux.unavailable.checking`.
//|---@class MuxStateErrorCodes: ErrorCode
//|---@field invalid MuxStateInvalidErrorCode `mux.state.invalid`.
//|---@field value_too_large MuxStateValueTooLargeErrorCode `mux.state.value_too_large`.
//|---@field unavailable MuxStateUnavailableErrorCode `mux.state.unavailable`.
//|---@class MuxObjectErrorCodes: ErrorCode
//|---@field invalid MuxObjectInvalidErrorCode `mux.object.invalid`.
//|---@field unavailable MuxObjectUnavailableErrorCode `mux.object.unavailable`.
//|---@class MuxFlagErrorCodes: ErrorCode
//|---@field invalid MuxFlagInvalidErrorCode `mux.flag.invalid`.
//|---@class MuxPowerErrorCodes: ErrorCode
//|---@field invalid MuxPowerInvalidErrorCode `mux.power.invalid`.
//|---@class MuxAccessErrorCodes: ErrorCode
//|---@field invalid MuxAccessInvalidErrorCode `mux.access.invalid`.
//|---@class MuxConnectionErrorCodes: ErrorCode
//|---@field invalid MuxConnectionInvalidErrorCode `mux.connection.invalid`.
//|---@field unavailable MuxConnectionUnavailableErrorCode `mux.connection.unavailable`.
//|---@class MuxChannelErrorCodes: ErrorCode
//|---@field invalid MuxChannelInvalidErrorCode `mux.channel.invalid`.
//|---@class MuxChannelFlagErrorCodes: ErrorCode
//|---@field invalid MuxChannelFlagInvalidErrorCode `mux.channel_flag.invalid`.
//|---@class MuxTextErrorCodes: ErrorCode
//|---@field invalid MuxTextInvalidErrorCode `mux.text.invalid`.
//|---@class MuxModuleErrorCodes: ErrorCode
//|---@field invalid MuxModuleInvalidErrorCode `mux.module.invalid`.
//|---@field unavailable MuxModuleUnavailableErrorCode `mux.module.unavailable`.
//|---@class MuxConfigErrorCodes: ErrorCode
//|---@field not_found MuxConfigNotFoundErrorCode `mux.config.not_found`.
//|---@field unsupported MuxConfigUnsupportedErrorCode `mux.config.unsupported`.
//|---@class MuxErrorCodes: ErrorCode
//|---@field arg MuxArgErrorCodes Invalid-argument code branch.
//|---@field unavailable MuxUnavailableErrorCodes Runtime-availability code branch.
//|---@field runtime MuxRuntimeErrorCode `mux.runtime`.
//|---@field state MuxStateErrorCodes Persistent-state code branch.
//|---@field object MuxObjectErrorCodes Database-object code branch.
//|---@field flag MuxFlagErrorCodes Object-flag code branch.
//|---@field power MuxPowerErrorCodes Object-power code branch.
//|---@field access MuxAccessErrorCodes Command-access code branch.
//|---@field connection MuxConnectionErrorCodes Connection code branch.
//|---@field channel MuxChannelErrorCodes Communication-channel code branch.
//|---@field channel_flag MuxChannelFlagErrorCodes Channel-flag code branch.
//|---@field text MuxTextErrorCodes Styled-text code branch.
//|---@field module MuxModuleErrorCodes Lua-module code branch.
//|---@field config MuxConfigErrorCodes Configuration code branch.
//|---@field internal MuxInternalErrorCode `mux.internal`.
// lua-types-end

// lua-types-begin mux 00045
//|---Checked `testing.assertion` error-code node used by the Lua test harness.
//|---@class TestingAssertionErrorCode: ErrorCode
//|---@field code "testing.assertion"
//|---Checked `testing.runtime` error-code node used by the Lua test harness.
//|---@class TestingRuntimeErrorCode: ErrorCode
//|---@field code "testing.runtime"
//|---Checked native code tree used by the Lua test harness.
//|---@class TestingErrorCodes: ErrorCode
//|---@field assertion TestingAssertionErrorCode `testing.assertion`.
//|---@field runtime TestingRuntimeErrorCode `testing.runtime`.
// lua-types-end

// lua-types-begin mux 00046
//|---Immutable lookup namespace for command-access constants.
//|---
//|---Raises [`mux.error.codes.access.invalid`](lua://mux.error.codes.access.invalid)
//|---for unknown or non-string keys and attempted mutation.
//|---@class AccessNamespace
//|---@field PUBLIC Access Allows every invoker; also the default when access is omitted.
//|---@field WIZARD Access Allows Wizards and God.
//|---@field GOD Access Allows only God.
//|---@see mux.error.codes.access.invalid
// lua-types-end

// lua-types-begin mux 00047
//|---Dynamic, immutable lookup namespace for registered flags. Keys must use the
//|---canonical uppercase native name.
//|---
//|---Raises [`mux.error.codes.flag.invalid`](lua://mux.error.codes.flag.invalid) for
//|---unknown or non-string keys and attempted mutation.
//|---@class FlagNamespace
//|---@field ANSI Flag Enables ANSI-capable output for the object.
//|---@field AUDIBLE Flag Enables sound-capable notifications associated with the object.
//|---@field AUDITORIUM Flag Applies auditorium-style speech propagation.
//|---@field BLIND Flag Marks the object as unable to see normally.
//|---@field CONNECTED Flag Marks a player as currently connected.
//|---@field DARK Flag Hides the object according to native visibility rules.
//|---@field FLOATING Flag Prevents ordinary location inheritance for the object.
//|---@field GAGGED Flag Prevents the object from speaking normally.
//|---@field GOING Flag Marks the object for deferred destruction.
//|---@field HALTED Flag Prevents queued command execution by the object.
//|---@field IN_CHARACTER Flag Marks the object as participating in in-character play.
//|---@field LIGHT Flag Makes the object visible through native light rules.
//|---@field MONITOR Flag Enables command monitoring behavior.
//|---@field NO_COMMAND Flag Excludes commands stored on the object from command matching.
//|---@field SAFE Flag Protects the object from ordinary destruction.
//|---@field SUSPECT Flag Marks a player for suspect-activity monitoring.
//|---@field TRANSPARENT Flag Allows visibility through the object.
//|---@field WIZARD Flag Grants Wizard status under native privilege rules.
//|---@field ZOMBIE Flag Allows a thing to act through its owner under native rules.
//|---@see mux.error.codes.flag.invalid
// lua-types-end

// lua-types-begin mux 00048
//|---Immutable namespace of typed native locks. Unknown or non-string lookups
//|---and attempted mutation raise
//|---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
//|---@class (exact) LockNamespace
//|---@field MATCH Lock Prefer an object that passes during key-aware matching.
//|---@field TRAVERSE Lock Traverse an exit.
//|---@field TAKE Lock Take an object.
//|---@field USE Lock Use an object.
//|---@field DROP Lock Drop an object.
//|---@field GIVE Lock Give an object.
//|---@field RECEIVE Lock Receive a given object.
//|---@field ENTER Lock Enter an object, room, BattleTech unit, bay, or hangar.
//|---@field LEAVE Lock Leave an object or room.
//|---@field TELEPORT Lock Teleport into a destination.
//|---@field TELEPORT_OUT Lock Teleport out of an origin.
//|---@field LINK Lock Link an exit or object.
//|---@field SET_HOME Lock Set an object's home to a destination.
//|---@field SPEAK Lock Speak in a location.
//|---@field CHANNEL_JOIN Lock Join a channel.
//|---@field CHANNEL_TRANSMIT Lock Transmit on a channel.
//|---@field CHANNEL_RECEIVE Lock Receive channel traffic.
//|---@field IDENTIFY_BUILDING Lock Silently identify a visible BattleTech structure.
//|---@field IDENTIFY_BUILDING Lock Identify a BattleTech building contact.
//|---@see mux.error.codes.arg.invalid
// lua-types-end

// lua-types-begin mux 00049
//|---Dynamic, immutable lookup namespace for registered powers. Keys must use the
//|---canonical uppercase native name.
//|---
//|---Raises [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid)
//|---for unknown or non-string keys and attempted mutation.
//|---@class PowerNamespace
//|---@field IDLE Power Exempts a player from ordinary idle-timeout handling.
//|---@see mux.error.codes.power.invalid
// lua-types-end

// lua-types-begin mux 00050
//|---Immutable namespace of typed native object kinds.
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for
//|---unknown or non-string keys and attempted mutation.
//|---@class (exact) ObjectTypeNamespace
//|---@field ROOM RoomObjectType Detached room kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
//|---@field THING ThingObjectType Contained thing kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
//|---@field EXIT ExitObjectType Attached exit kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
//|---@field PLAYER PlayerObjectType Player kind; existing players may have this type, but scripts cannot create them.
//|---@see mux.error.codes.arg.invalid
// lua-types-end

// lua-types-begin mux 00051
//|---The native MUX host API.
//|---@class MuxPackage
//|---@field macro MuxMacroPackage Trusted macro administration.
//|---@field comsys MuxComsysPackage Trusted live communication-channel administration.
//|---@field config MuxConfigPackage Read-only scalar server configuration.
//|---@field error MuxErrorPackage Structured errors and checked code nodes.
//|---@field session MuxSessionPackage Live connections and interactive flows.
//|---@field telnet MuxTelnetPackage Telnet protocol state and capabilities.
//|---@field text MuxTextPackage Styled-text utilities.
//|---@field world MuxWorldPackage World database object access.
//|mux = {}
// lua-types-end

// lua-types-begin mux 00052
//|---Trusted access to the live communication-channel registry. Mutations take
//|---effect immediately and are not rolled back when the surrounding Lua
//|---callback later fails.
//|---@class MuxComsysPackage
//|---@field flags ChannelFlagNamespace Immutable typed channel-flag constants.
//|local mux_comsys = {}
// lua-types-end

// lua-types-begin mux 00053
//|---Read-only access to live scalar server configuration.
//|---@class MuxConfigPackage
//|local mux_config = {}
// lua-types-end

// lua-types-begin mux 00054
//|---@class MuxErrorPackage
//|---@field codes MuxErrorCodes Checked native `mux` code tree.
//|local mux_error = {}
// lua-types-end

// lua-types-begin mux 00055
//|---Live connection queries and interactive flows.
//|---@class MuxSessionPackage
//|local mux_session = {}
// lua-types-end

// lua-types-begin mux 00056
//|---Telnet protocol state and capabilities for live connections.
//|---@class MuxTelnetPackage
//|local mux_telnet = {}
// lua-types-end

// lua-types-begin mux 00057
//|---Styled-text validation, construction, inspection, and transformation.
//|---@class MuxTextPackage
//|local mux_text = {}
// lua-types-end

// lua-types-begin mux 00058
//|---World database object access.
//|---@class MuxWorldPackage
//|---@field access AccessNamespace Immutable namespace of command-access constants.
//|---@field flags FlagNamespace Immutable namespace of registered flag constants.
//|---@field locks LockNamespace Immutable namespace of native lock constants.
//|---@field powers PowerNamespace Immutable namespace of registered power constants.
//|---@field types ObjectTypeNamespace Immutable namespace of native object-kind constants.
//|local mux_world = {}
// lua-types-end

// lua-types-begin mux 00115
//|---Checks the database for inconsistencies and repairs damage found by the
//|---default native `@dbck` pass. Findings are written to the server log.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
//|---@see mux.error.codes.unavailable.checking
//|function mux.check_db() end
// lua-types-end

// lua-types-begin mux 00129
//|---Appends a newline-terminated message to a permitted file under `game/logs`.
//|---@param filename string
//|---@param message string
//|---@return boolean written
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|function mux.log(filename, message) end
// lua-types-end

// lua-types-begin mux 00149
//|mux.config = mux_config
//|mux.comsys = mux_comsys
//|mux.error = mux_error
//|mux.session = mux_session
//|mux.telnet = mux_telnet
//|mux.text = mux_text
//|mux.world = mux_world
// lua-types-end

// lua-types-begin mux 00173
//|return mux
// lua-types-end

// lua-types-begin mux 00172
//|---A command declaration registered when a game module loads.
//|---@class MuxCommandDefinition
//|---@field name string Canonical command token, without whitespace or slash.
//|---@field permission 'everyone'|'wizard'|'god' Required actor authority; restricted entries are skipped.
//|---@field pattern string Lua string.find pattern.
//|---@field handler fun(ctx: table, ...): boolean? True consumes the input; false/nil continues dispatch.
// lua-types-end
