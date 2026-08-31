---@meta _

---Maintained by `just update-lua-types`; edit the native bindings and their
---Doxygen comments, then refresh this definition instead of editing it alone.

---@alias DbRef integer Database object reference.
---@alias StateValue string|boolean|number Scalar value supported by persistent object state.
---@alias TelnetEnvironmentKind "var"|"uservar" RFC 1572 NEW-ENVIRON variable namespace.
---@alias ObjectType "room"|"thing"|"exit"|"player" Public native database object kind.
---@alias NativeErrorRoot "mux"|"btech"|"testing" Root of a checked native error-code tree.
---@alias ConfigValue string|number|boolean Scalar value returned by the live configuration registry.

---A checked error-code symbol. Calling `tostring` returns its dotted `code`.
---@class ErrorCode
---@field code string Fully qualified error code represented by this node.

---A structured Lua failure raised by native and script APIs. Calling `tostring`
---returns its human-readable message.
---@class Error
---@field code string Stable dotted error code.
---@field message string Human-readable failure description.
---@field detail? any Optional structured context.
---@field cause? any Earlier failure preserved by wrapping.
---@field traceback? string Traceback added by [`mux.error.pcall`](lua://mux.error.pcall).
local Error = {}

---Tests this error's code using dotted-prefix matching.
---@param code string|ErrorCode
---@return boolean matches
function Error:is(code) end

---Returns the deepest table-valued cause, or this error when it has none.
---@return any root
function Error:root() end

---@class ErrorFields
---@field code string|ErrorCode
---@field message string
---@field detail? any
---@field cause? any

---@class MuxArgErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.arg.invalid`.
---@class MuxUnavailableErrorCodes: ErrorCode
---@field checking ErrorCode `mux.unavailable.checking`.
---@class MuxStateErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.state.invalid`.
---@field value_too_large ErrorCode `mux.state.value_too_large`.
---@field unavailable ErrorCode `mux.state.unavailable`.
---@class MuxObjectErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.object.invalid`.
---@field unavailable ErrorCode `mux.object.unavailable`.
---@class MuxAttributeErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.attribute.invalid`.
---@class MuxFlagErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.flag.invalid`.
---@class MuxPowerErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.power.invalid`.
---@class MuxAccessErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.access.invalid`.
---@class MuxConnectionErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.connection.invalid`.
---@field unavailable ErrorCode `mux.connection.unavailable`.
---@class MuxTextErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.text.invalid`.
---@class MuxModuleErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.module.invalid`.
---@field unavailable ErrorCode `mux.module.unavailable`.
---@class MuxConfigErrorCodes: ErrorCode
---@field not_found ErrorCode `mux.config.not_found`.
---@field unsupported ErrorCode `mux.config.unsupported`.
---@class MuxErrorCodes: ErrorCode
---@field arg MuxArgErrorCodes
---@field unavailable MuxUnavailableErrorCodes
---@field runtime ErrorCode `mux.runtime`.
---@field state MuxStateErrorCodes
---@field object MuxObjectErrorCodes
---@field attribute MuxAttributeErrorCodes
---@field flag MuxFlagErrorCodes
---@field power MuxPowerErrorCodes
---@field access MuxAccessErrorCodes
---@field connection MuxConnectionErrorCodes
---@field text MuxTextErrorCodes
---@field module MuxModuleErrorCodes
---@field config MuxConfigErrorCodes
---@field internal ErrorCode `mux.internal`.

---@class ErrorCodeTree: ErrorCode
---@field [string] ErrorCodeTree

---Checked native codes used by the Lua test harness.
---@class TestingErrorCodes: ErrorCode
---@field assertion ErrorCode `testing.assertion`.
---@field runtime ErrorCode `testing.runtime`.

---Persistent state entry returned by [`State:entries`](lua://State.entries).
---@class StateEntry
---@field key string
---@field value StateValue

---A persistent, object-scoped state namespace with a native string representation.
---@class State
local State = {}

---Gets a stored value, an optional default, or nil.
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
---@generic T
---@param key string
---@param default? T
---@return StateValue|T|nil value
---@see mux.error.codes.object.invalid
---@see mux.error.codes.state.invalid
function State:get(key, default) end

---Tests whether a state key is present.
---@param key string
---@return boolean exists
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.state.invalid
function State:has(key) end

---Sets a supported value, or deletes the key when `value` is nil.
---
---Raises invalid-object/key/value errors or [`mux.error.codes.state.value_too_large`](lua://mux.error.codes.state.value_too_large).
---@param key string
---@param value? StateValue
---@see mux.error.codes.object.invalid
---@see mux.error.codes.state.invalid
---@see mux.error.codes.state.value_too_large
function State:set(key, value) end

---Deletes a state key and reports whether it existed.
---@param key string
---@return boolean existed
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.state.invalid
function State:delete(key) end

---Lists keys sorted in native key order.
---
---Raises [`mux.error.codes.state.unavailable`](lua://mux.error.codes.state.unavailable) outside a callback transaction or if state changes while enumerating.
---@return string[] keys
---@see mux.error.codes.object.invalid
---@see mux.error.codes.state.unavailable
function State:keys() end

---Lists key/value records sorted by key.
---@return StateEntry[] entries
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.unavailable`](lua://mux.error.codes.state.unavailable).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.state.unavailable
function State:entries() end

---Returns only the requested keys that are present.
---@param keys string[]
---@return table<string, StateValue> values
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.state.invalid
function State:get_many(keys) end

---Applies several state updates; nil values delete keys when supplied by Lua iteration.
---@param values table<string, StateValue|nil>
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid), [`mux.error.codes.state.value_too_large`](lua://mux.error.codes.state.value_too_large).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.state.invalid
---@see mux.error.codes.state.value_too_large
function State:set_many(values) end

---A handle exposing supported native attributes for one object.
---@class Attribute
local Attribute = {}

---A checked flag constant obtained from [`mux.world.flags`](lua://mux.world.flags).
---Its string form is the canonical uppercase native name, and equality compares
---the native flag identity within the current runtime.
---@class Flag

---A checked power constant obtained from [`mux.world.powers`](lua://mux.world.powers).
---Its string form is the canonical uppercase native name, and equality compares
---the native power identity within the current runtime.
---@class Power

---A checked command-access constant obtained from [`mux.world.access`](lua://mux.world.access).
---Its string form is its uppercase name, and equality compares access identity.
---@class Access

---A typed native lock obtained from [`mux.world.locks`](lua://mux.world.locks).
---Its string form is its uppercase name, and equality compares lock identity
---within the current runtime.
---@class Lock

---Immutable namespace of typed native locks. Unknown or non-string lookups
---and attempted mutation raise
---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@class (exact) LockNamespace
---@field MATCH Lock Prefer an object that passes during key-aware matching.
---@field TRAVERSE Lock Traverse an exit.
---@field TAKE Lock Take an object.
---@field USE Lock Use an object.
---@field DROP Lock Drop an object.
---@field GIVE Lock Give an object.
---@field RECEIVE Lock Receive a given object.
---@field ENTER Lock Enter an object, room, BattleTech unit, bay, or hangar.
---@field LEAVE Lock Leave an object or room.
---@field TELEPORT Lock Teleport into a destination.
---@field TELEPORT_OUT Lock Teleport out of an origin.
---@field LINK Lock Link an exit or object.
---@field SET_HOME Lock Set an object's home to a destination.
---@field SPEAK Lock Speak in a location.
---@field CHANNEL_JOIN Lock Join a channel.
---@field CHANNEL_TRANSMIT Lock Transmit on a channel.
---@field CHANNEL_RECEIVE Lock Receive channel traffic.
---@field IDENTIFY_BUILDING Lock Identify a BattleTech building contact.
---@see mux.error.codes.arg.invalid

---Immutable lookup namespace for command-access constants.
---
---Raises [`mux.error.codes.access.invalid`](lua://mux.error.codes.access.invalid)
---for unknown or non-string keys and attempted mutation.
---@class AccessNamespace
---@field PUBLIC Access Allows every invoker; also the default when access is omitted.
---@field WIZARD Access Allows Wizards and God.
---@field GOD Access Allows only God.
---@see mux.error.codes.access.invalid

---Dynamic, immutable lookup namespace for registered flags. Keys must use the
---canonical uppercase native name.
---
---Raises [`mux.error.codes.flag.invalid`](lua://mux.error.codes.flag.invalid) for
---unknown or non-string keys and attempted mutation.
---@class FlagNamespace
---@field ANSI Flag
---@field AUDIBLE Flag
---@field AUDITORIUM Flag
---@field BLIND Flag
---@field CONNECTED Flag
---@field DARK Flag
---@field FLOATING Flag
---@field GAGGED Flag
---@field GOING Flag
---@field HALTED Flag
---@field IN_CHARACTER Flag
---@field LIGHT Flag
---@field MONITOR Flag
---@field NO_COMMAND Flag
---@field SAFE Flag
---@field SUSPECT Flag
---@field TRANSPARENT Flag
---@field WIZARD Flag
---@field XCODE Flag
---@field ZOMBIE Flag
---@see mux.error.codes.flag.invalid

---Dynamic, immutable lookup namespace for registered powers. Keys must use the
---canonical uppercase native name.
---
---Raises [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid)
---for unknown or non-string keys and attempted mutation.
---@class PowerNamespace
---@field IDLE Power
---@see mux.error.codes.power.invalid

---A generation-checked view of the flags set on one object.
---@class Flags
local Flags = {}

---Lists set flags in native registry order.
---@return Flag[] values
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.object.invalid
function Flags:list() end

---Tests whether this object has a flag.
---@param flag Flag Checked constant from [`mux.world.flags`](lua://mux.world.flags).
---@return boolean present
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.flag.invalid`](lua://mux.error.codes.flag.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.flag.invalid
function Flags:has(flag) end

---Adds a flag and reports whether the object changed.
---@param flag Flag Checked constant from [`mux.world.flags`](lua://mux.world.flags).
---@return boolean changed
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.flag.invalid`](lua://mux.error.codes.flag.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.flag.invalid
---@see mux.error.codes.object.unavailable
function Flags:add(flag) end

---Removes a flag and reports whether the object changed.
---@param flag Flag Checked constant from [`mux.world.flags`](lua://mux.world.flags).
---@return boolean changed
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.flag.invalid`](lua://mux.error.codes.flag.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.flag.invalid
---@see mux.error.codes.object.unavailable
function Flags:remove(flag) end

---A generation-checked view of the powers granted to one object.
---@class Powers
local Powers = {}

---Lists granted powers in native registry order.
---@return Power[] values
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.object.invalid
function Powers:list() end

---Tests whether this object has a power.
---@param power Power Checked constant from [`mux.world.powers`](lua://mux.world.powers).
---@return boolean present
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.power.invalid
function Powers:has(power) end

---Grants a power and reports whether the object changed.
---@param power Power Checked constant from [`mux.world.powers`](lua://mux.world.powers).
---@return boolean changed
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), or [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.power.invalid
function Powers:add(power) end

---Removes a power and reports whether the object changed.
---@param power Power Checked constant from [`mux.world.powers`](lua://mux.world.powers).
---@return boolean changed
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), or [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.power.invalid
function Powers:remove(power) end

---Gets a raw native attribute, or nil when unset.
---@param name string
---@return string? value
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.attribute.invalid`](lua://mux.error.codes.attribute.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.attribute.invalid
function Attribute:get(name) end

---Sets a raw native attribute, or clears it with nil.
---@param name string
---@param value? string
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.attribute.invalid`](lua://mux.error.codes.attribute.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.attribute.invalid
function Attribute:set(name, value) end

---Returns every supported native attribute; unset values appear as empty strings.
---@return table<string, string> entries
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.object.invalid
function Attribute:entries() end

---A generation-checked native database object handle. Native equality compares
---object identity, and `tostring` returns its database-reference form.
---@class Object
local Object = {}

---Returns this object's native database reference.
---@return DbRef dbref
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.object.invalid
function Object:dbref() end

---Returns this object's native object type.
---@return ObjectType? type
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.object.invalid
function Object:type() end

---Returns this object's current stored name.
---@return string name
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.object.invalid
function Object:name() end

---Changes this object's name using native object-name validation.
---@param name string New UTF-8 name, optionally containing styled-text markup.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function Object:set_name(name) end

---Returns directly contained objects in database order.
---@return Object[] contents
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:contents() end

---Tests whether a directly contained member is visible to a viewer.
---@param viewer DbRef|Object
---@param member DbRef|Object
---@return boolean visible
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:contents_visible(viewer, member) end

---Returns directly attached exits in database order.
---@return Object[] exits
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:exits() end

---Tests whether a directly attached exit is visible to a viewer.
---@param viewer DbRef|Object
---@param exit DbRef|Object
---@return boolean visible
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:exits_visible(viewer, exit) end

---Returns this object's assigned zone, or nil when no zone is assigned or the zone is being destroyed.
---@return Object? zone Assigned zone.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:zone() end

---Assigns this object's zone, or clears it when `zone` is nil.
---@param zone DbRef|Object|nil Live thing or room to assign, or nil to clear the zone. This argument must be supplied explicitly.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `zone` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function Object:set_zone(zone) end

---Returns this object's assigned affiliation, or nil when none is assigned or the affiliate is being destroyed.
---@return Object? affiliation Assigned affiliation.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) for an invalid receiver or stored affiliation.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:affiliation() end

---Assigns this object's affiliation, or clears it when `affiliation` is nil.
---@param affiliation DbRef|Object|nil Any live object to assign, or nil to clear the affiliation. This argument must be supplied explicitly.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `affiliation` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) for an invalid reference, or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable) when either object is being destroyed.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function Object:set_affiliation(affiliation) end

---Returns this object's direct Lua parent path, or nil when none is assigned.
---@return string? parent `object_logic`-relative parent path.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:lua_parent() end

---Assigns this object's direct Lua parent path, or clears it when `parent` is nil.
---@param parent string|nil Existing `object_logic`-relative `.lua` path, or nil to clear it. This argument must be supplied explicitly.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `parent` is omitted or malformed, [`mux.error.codes.module.invalid`](lua://mux.error.codes.module.invalid) for an invalid or unavailable path, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.module.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function Object:set_lua_parent(parent) end

---Creates a persistent-state handle for an exact, case-sensitive namespace.
---@param namespace string
---@return State state
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.state.invalid
function Object:state(namespace) end

---Creates a native-attribute handle for this object.
---@return Attribute attributes
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
function Object:attributes() end

---Creates a handle for this object's flags.
---@return Flags flags
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
function Object:flags() end

---Creates a handle for this object's powers.
---@return Powers powers
---
---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
---@see mux.error.codes.object.invalid
---@see mux.error.codes.unavailable.checking
function Object:powers() end

---One player connection visible to the ordinary WHO command.
---@class Connection
---@field object Object Connected player.
---@field name string Current object name.
---@field connected_for integer Connected duration in seconds.
---@field idle_for integer Idle duration in seconds.

---Non-privileged server population statistics.
---@class WhoSummary
---@field hidden integer Hidden-player count; currently always zero for this non-privileged view.
---@field record integer Record simultaneous-player count.
---@field maximum? integer Configured limit, or nil when unlimited.

---Telnet protocol state and capabilities for live connections.
---@class MuxTelnetPackage
local mux_telnet = {}

---Tests whether a binary-safe RFC 1572 NEW-ENVIRON variable is defined.
---@param descriptor integer Live descriptor ID, normally `ctx.descriptor`.
---@param kind TelnetEnvironmentKind NEW-ENVIRON variable namespace.
---@param name string Binary-safe variable name.
---@return boolean defined Whether the variable is present, including with an empty value.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.connection.invalid
function mux_telnet.environment_has(descriptor, kind, name) end

---Gets a binary-safe RFC 1572 NEW-ENVIRON value.
---@param descriptor integer Live descriptor ID, normally `ctx.descriptor`.
---@param kind TelnetEnvironmentKind NEW-ENVIRON variable namespace.
---@param name string Binary-safe variable name.
---@return string? value Binary-safe value, or nil when the variable is absent.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.connection.invalid
function mux_telnet.environment_get(descriptor, kind, name) end

---Optional styled-text attributes applied by [`mux.text.style`](lua://mux.text.style).
---@class StyleOptions
---@field foreground? string Palette foreground name.
---@field background? string Palette background name.
---@field bold? boolean
---@field underline? boolean
---@field inverse? boolean

---@class MuxErrorPackage
---@field codes MuxErrorCodes Checked native `mux` code tree.
local mux_error = {}

---Creates a structured error without raising it.
---@param fields ErrorFields
---@return Error error
---
---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@see mux.error.codes.arg.invalid
function mux_error.new(fields) end

---Raises a structured error with the requested code.
---@param code string|ErrorCode
---@param message string
---@param detail? any
---
---Raises the requested code, or [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `code` is invalid.
---@see mux.error.codes.arg.invalid
function mux_error.raise(code, message, detail) end

---Tests a table's code using exact or dotted-prefix matching.
---@param value any
---@param code string|ErrorCode
---@return boolean matches
---
---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@see mux.error.codes.arg.invalid
function mux_error.is(value, code) end

---Returns a truthy value unchanged or raises `err` unchanged.
---@generic T
---@param value T
---@param err any
---@return T value
---
---Raises `err` unchanged when `value` is false or nil.
function mux_error.check(value, err) end

---Wraps a failure as the cause of a new structured error.
---@param err any
---@param code string|ErrorCode
---@param message string
---@return Error error
---
---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.runtime`](lua://mux.error.codes.runtime).
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.runtime
function mux_error.wrap(err, code, message) end

---Calls a function, returning all results on success or a normalized traced error.
---@generic R...
---@param fn fun(...): R...
---@param ... any
---@return true, R...
---@overload fun(fn: function, ...: any): false, Error
function mux_error.pcall(fn, ...) end

---Builds a checked code-symbol tree for an author-defined namespace.
---@param prefix string
---@param names string[]
---@return ErrorCodeTree codes
---
---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@see mux.error.codes.arg.invalid
function mux_error.namespace(prefix, names) end

---Returns the cached checked native code tree for a root.
---@param root NativeErrorRoot
---@return ErrorCodeTree codes
---@overload fun(root: "mux"): MuxErrorCodes
---@overload fun(root: "btech"): BtechErrorCodes
---@overload fun(root: "testing"): TestingErrorCodes
---
---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@see mux.error.codes.arg.invalid
function mux_error.code_tree(root) end

---Read-only access to live scalar server configuration.
---@class MuxConfigPackage
local mux_config = {}

---Returns the live scalar value of an exact, case-sensitive configuration directive.
---@param name string Configuration directive name; embedded NUL bytes are rejected.
---@return ConfigValue value Current value represented by its native Lua scalar type.
---
---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.config.not_found`](lua://mux.error.codes.config.not_found), [`mux.error.codes.config.unsupported`](lua://mux.error.codes.config.unsupported), or [`mux.error.codes.internal`](lua://mux.error.codes.internal).
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.config.not_found
---@see mux.error.codes.config.unsupported
---@see mux.error.codes.internal
function mux_config.get(name) end

---Fields accepted when creating a detached room.
---@class (exact) CreateRoomOptions
---@field name string Required UTF-8 name, optionally containing valid styled-text markup.

---Fields accepted when creating and placing a thing.
---@class (exact) CreateThingOptions
---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
---@field location DbRef|Object Required object that can contain the new thing.
---@field home? DbRef|Object Home object; defaults to `location` when omitted.

---Fields accepted when creating and attaching an exit.
---@class (exact) CreateExitOptions
---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
---@field location DbRef|Object Required source object capable of holding exits.
---@field destination? DbRef|Object Optional destination capable of containing objects; omission leaves the exit unlinked.

---Fields accepted when teleporting a thing or player.
---@class (exact) TeleportOptions
---@field object DbRef|Object Required thing or player to move.
---@field destination DbRef|Object Required object capable of containing objects.

---Options controlling object destruction.
---@class (exact) DestroyOptions
---@field override? boolean Whether to bypass the target's SAFE flag; core objects and Wizard players remain protected.

---Fields selecting a native lock invocation to test.
---@class (exact) LockPassesOptions
---@field object DbRef|Object Required object whose lock is tested.
---@field enactor DbRef|Object Required object attempting the action.
---@field lock Lock Required typed lock constant from [`mux.world.locks`](lua://mux.world.locks).
---@field cause? DbRef|Object Object that caused the action; defaults to `enactor`.
---@field subject? DbRef|Object Object acted upon in the lock context; defaults to `enactor`.

---World database object access.
---@class MuxWorldPackage
---@field access AccessNamespace Immutable namespace of command-access constants.
---@field flags FlagNamespace Immutable namespace of registered flag constants.
---@field locks LockNamespace Immutable namespace of native lock constants.
---@field powers PowerNamespace Immutable namespace of registered power constants.
local mux_world = {}

---Creates a validated object handle from a dbref or existing handle.
---@param dbref DbRef|Object
---@return Object object
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function mux_world.object(dbref) end

---Creates a detached room with the configured room flags and default Lua parent.
---@param options CreateRoomOptions Creation fields; unknown fields are rejected.
---@return Object room Newly created room.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.unavailable
function mux_world.create_room(options) end

---Creates a thing, establishes its home, and places it in a container.
---@param options CreateThingOptions Creation fields; unknown fields are rejected.
---@return Object thing Newly created thing.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function mux_world.create_thing(options) end

---Creates an exit, attaches it to a source, and optionally links it to a destination.
---@param options CreateExitOptions Creation fields; unknown fields are rejected.
---@return Object exit Newly created exit.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function mux_world.create_exit(options) end

---Links an exit to a destination, or unlinks it when `destination` is nil.
---@param exit DbRef|Object Live exit to update.
---@param destination DbRef|Object|nil Live object capable of containing objects, or nil to unlink the exit.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `destination` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function mux_world.link_exit(exit, destination) end

---Teleports a thing or player through the native movement path.
---@param options TeleportOptions Teleport fields; unknown fields are rejected.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function mux_world.teleport(options) end

---Silently schedules a live object for destruction by the normal maintenance purge.
---@param object DbRef|Object Object to destroy.
---@param options? DestroyOptions Destruction controls; unknown fields are rejected.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable), or [`mux.error.codes.internal`](lua://mux.error.codes.internal) for an unexpected native destruction result.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
---@see mux.error.codes.internal
function mux_world.destroy(object, options) end

---Tests a native object lock without emitting lock messages or performing the
---associated action. The lock runs with a silent callback context.
---@param options LockPassesOptions Lock invocation fields; unknown fields are rejected.
---@return boolean passes Whether the selected lock passes.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function mux_world.lock_passes(options) end

---Live connection queries and interactive flows.
---@class MuxSessionPackage
local mux_session = {}

---Lists connected players visible to the ordinary WHO command.
---@return Connection[] players
function mux_session.connected_players() end

---Returns the non-privileged WHO summary.
---@return WhoSummary summary
function mux_session.who_summary() end

---Attaches an interactive flow to a descriptor and displays its first prompt.
---@param descriptor integer
---@param module string Require-style module path.
---@param first_step string Key in the module's `flows` table.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.unavailable`](lua://mux.error.codes.connection.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.connection.unavailable
function mux_session.flow_start(descriptor, module, first_step) end

---Styled-text validation, construction, inspection, and transformation.
---@class MuxTextPackage
local mux_text = {}

---Validates styled-text markup and returns it unchanged.
---@param value string
---@return string markup
---
---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
---@see mux.error.codes.text.invalid
function mux_text.markup(value) end

---Tests whether every byte is printable ASCII (0x20 through 0x7e).
---@param value string
---@return boolean printable
function mux_text.is_printable_ascii(value) end

---Wraps text in markup described by the supplied style options.
---@param value string
---@param options StyleOptions
---@return string styled
---
---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
---@see mux.error.codes.text.invalid
function mux_text.style(value, options) end

---Removes styled-text markup and ANSI styling.
---@param value string
---@return string plain
function mux_text.strip_style(value) end

---Measures visible byte width, excluding markup and ANSI styling.
---@param value string
---@return integer width
function mux_text.width(value) end

---Safely truncates styled text to a non-negative visible byte width.
---@param value string
---@param width integer
---@return string truncated
---
---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
---@see mux.error.codes.text.invalid
function mux_text.truncate(value, width) end

---The native MUX host API.
---@class MuxPackage
---@field config MuxConfigPackage Read-only scalar server configuration.
---@field error MuxErrorPackage
---@field session MuxSessionPackage Live connections and interactive flows.
---@field telnet MuxTelnetPackage Telnet protocol state and capabilities.
---@field text MuxTextPackage Styled-text utilities.
---@field world MuxWorldPackage World database object access.
mux = {}

---Appends a newline-terminated message to a permitted file under `game/logs`.
---@param filename string
---@param message string
---@return boolean written
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
function mux.log(filename, message) end

---Sends valid UTF-8 text to an object.
---@param object DbRef|Object
---@param message string
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.connection.invalid
---@see mux.error.codes.object.invalid
function mux_world.pemit(object, message) end

mux.config = mux_config
mux.error = mux_error
mux.session = mux_session
mux.telnet = mux_telnet
mux.text = mux_text
mux.world = mux_world

return mux
