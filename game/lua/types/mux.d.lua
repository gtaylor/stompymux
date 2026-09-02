---@meta _

---Maintained by `just update-lua-types`; refresh this definition from the
---native bindings and their Doxygen comments rather than editing it alone.

---@alias DbRef integer Database object reference.
---@alias StateValue string|boolean|number Scalar value supported by persistent object state.
---@alias TelnetEnvironmentKind "var"|"uservar" RFC 1572 NEW-ENVIRON variable namespace.
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
---@field code string|ErrorCode Stable dotted code or checked code node.
---@field message string Human-readable failure description.
---@field detail? any Optional structured context.
---@field cause? any Optional earlier failure.

---Checked `mux.arg.invalid` error-code node.
---@class MuxArgInvalidErrorCode: ErrorCode
---@field code "mux.arg.invalid"
---Checked `mux.unavailable.checking` error-code node.
---@class MuxCheckingUnavailableErrorCode: ErrorCode
---@field code "mux.unavailable.checking"
---Checked `mux.runtime` error-code node.
---@class MuxRuntimeErrorCode: ErrorCode
---@field code "mux.runtime"
---Checked `mux.state.invalid` error-code node.
---@class MuxStateInvalidErrorCode: ErrorCode
---@field code "mux.state.invalid"
---Checked `mux.state.value_too_large` error-code node.
---@class MuxStateValueTooLargeErrorCode: ErrorCode
---@field code "mux.state.value_too_large"
---Checked `mux.state.unavailable` error-code node.
---@class MuxStateUnavailableErrorCode: ErrorCode
---@field code "mux.state.unavailable"
---Checked `mux.object.invalid` error-code node.
---@class MuxObjectInvalidErrorCode: ErrorCode
---@field code "mux.object.invalid"
---Checked `mux.object.unavailable` error-code node.
---@class MuxObjectUnavailableErrorCode: ErrorCode
---@field code "mux.object.unavailable"
---Checked `mux.attribute.invalid` error-code node.
---@class MuxAttributeInvalidErrorCode: ErrorCode
---@field code "mux.attribute.invalid"
---Checked `mux.flag.invalid` error-code node.
---@class MuxFlagInvalidErrorCode: ErrorCode
---@field code "mux.flag.invalid"
---Checked `mux.power.invalid` error-code node.
---@class MuxPowerInvalidErrorCode: ErrorCode
---@field code "mux.power.invalid"
---Checked `mux.access.invalid` error-code node.
---@class MuxAccessInvalidErrorCode: ErrorCode
---@field code "mux.access.invalid"
---Checked `mux.connection.invalid` error-code node.
---@class MuxConnectionInvalidErrorCode: ErrorCode
---@field code "mux.connection.invalid"
---Checked `mux.connection.unavailable` error-code node.
---@class MuxConnectionUnavailableErrorCode: ErrorCode
---@field code "mux.connection.unavailable"
---Checked `mux.channel.invalid` error-code node.
---@class MuxChannelInvalidErrorCode: ErrorCode
---@field code "mux.channel.invalid"
---Checked `mux.channel_flag.invalid` error-code node.
---@class MuxChannelFlagInvalidErrorCode: ErrorCode
---@field code "mux.channel_flag.invalid"
---Checked `mux.text.invalid` error-code node.
---@class MuxTextInvalidErrorCode: ErrorCode
---@field code "mux.text.invalid"
---Checked `mux.module.invalid` error-code node.
---@class MuxModuleInvalidErrorCode: ErrorCode
---@field code "mux.module.invalid"
---Checked `mux.module.unavailable` error-code node.
---@class MuxModuleUnavailableErrorCode: ErrorCode
---@field code "mux.module.unavailable"
---Checked `mux.config.not_found` error-code node.
---@class MuxConfigNotFoundErrorCode: ErrorCode
---@field code "mux.config.not_found"
---Checked `mux.config.unsupported` error-code node.
---@class MuxConfigUnsupportedErrorCode: ErrorCode
---@field code "mux.config.unsupported"
---Checked `mux.internal` error-code node.
---@class MuxInternalErrorCode: ErrorCode
---@field code "mux.internal"

---@class MuxArgErrorCodes: ErrorCode
---@field invalid MuxArgInvalidErrorCode `mux.arg.invalid`.
---@class MuxUnavailableErrorCodes: ErrorCode
---@field checking MuxCheckingUnavailableErrorCode `mux.unavailable.checking`.
---@class MuxStateErrorCodes: ErrorCode
---@field invalid MuxStateInvalidErrorCode `mux.state.invalid`.
---@field value_too_large MuxStateValueTooLargeErrorCode `mux.state.value_too_large`.
---@field unavailable MuxStateUnavailableErrorCode `mux.state.unavailable`.
---@class MuxObjectErrorCodes: ErrorCode
---@field invalid MuxObjectInvalidErrorCode `mux.object.invalid`.
---@field unavailable MuxObjectUnavailableErrorCode `mux.object.unavailable`.
---@class MuxAttributeErrorCodes: ErrorCode
---@field invalid MuxAttributeInvalidErrorCode `mux.attribute.invalid`.
---@class MuxFlagErrorCodes: ErrorCode
---@field invalid MuxFlagInvalidErrorCode `mux.flag.invalid`.
---@class MuxPowerErrorCodes: ErrorCode
---@field invalid MuxPowerInvalidErrorCode `mux.power.invalid`.
---@class MuxAccessErrorCodes: ErrorCode
---@field invalid MuxAccessInvalidErrorCode `mux.access.invalid`.
---@class MuxConnectionErrorCodes: ErrorCode
---@field invalid MuxConnectionInvalidErrorCode `mux.connection.invalid`.
---@field unavailable MuxConnectionUnavailableErrorCode `mux.connection.unavailable`.
---@class MuxChannelErrorCodes: ErrorCode
---@field invalid MuxChannelInvalidErrorCode `mux.channel.invalid`.
---@class MuxChannelFlagErrorCodes: ErrorCode
---@field invalid MuxChannelFlagInvalidErrorCode `mux.channel_flag.invalid`.
---@class MuxTextErrorCodes: ErrorCode
---@field invalid MuxTextInvalidErrorCode `mux.text.invalid`.
---@class MuxModuleErrorCodes: ErrorCode
---@field invalid MuxModuleInvalidErrorCode `mux.module.invalid`.
---@field unavailable MuxModuleUnavailableErrorCode `mux.module.unavailable`.
---@class MuxConfigErrorCodes: ErrorCode
---@field not_found MuxConfigNotFoundErrorCode `mux.config.not_found`.
---@field unsupported MuxConfigUnsupportedErrorCode `mux.config.unsupported`.
---@class MuxErrorCodes: ErrorCode
---@field arg MuxArgErrorCodes Invalid-argument code branch.
---@field unavailable MuxUnavailableErrorCodes Runtime-availability code branch.
---@field runtime MuxRuntimeErrorCode `mux.runtime`.
---@field state MuxStateErrorCodes Persistent-state code branch.
---@field object MuxObjectErrorCodes Database-object code branch.
---@field attribute MuxAttributeErrorCodes Native-attribute code branch.
---@field flag MuxFlagErrorCodes Object-flag code branch.
---@field power MuxPowerErrorCodes Object-power code branch.
---@field access MuxAccessErrorCodes Command-access code branch.
---@field connection MuxConnectionErrorCodes Connection code branch.
---@field channel MuxChannelErrorCodes Communication-channel code branch.
---@field channel_flag MuxChannelFlagErrorCodes Channel-flag code branch.
---@field text MuxTextErrorCodes Styled-text code branch.
---@field module MuxModuleErrorCodes Lua-module code branch.
---@field config MuxConfigErrorCodes Configuration code branch.
---@field internal MuxInternalErrorCode `mux.internal`.

---@class ErrorCodeTree: ErrorCode
---@field [string] ErrorCodeTree Checked child code segment.

---Checked `testing.assertion` error-code node used by the Lua test harness.
---@class TestingAssertionErrorCode: ErrorCode
---@field code "testing.assertion"
---Checked `testing.runtime` error-code node used by the Lua test harness.
---@class TestingRuntimeErrorCode: ErrorCode
---@field code "testing.runtime"
---Checked native code tree used by the Lua test harness.
---@class TestingErrorCodes: ErrorCode
---@field assertion TestingAssertionErrorCode `testing.assertion`.
---@field runtime TestingRuntimeErrorCode `testing.runtime`.

---Persistent state entry returned by [`State:entries`](lua://State.entries).
---@class StateEntry
---@field key string Stored state key.
---@field value StateValue Stored scalar value.

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

---A typed native object kind obtained from [`mux.world.types`](lua://mux.world.types).
---Its string form is its uppercase name, and equality compares native type
---identity within the current runtime.
---@class ObjectType

---The `ROOM` object-kind constant from the current runtime.
---@class RoomObjectType: ObjectType

---The `THING` object-kind constant from the current runtime.
---@class ThingObjectType: ObjectType

---The `EXIT` object-kind constant from the current runtime.
---@class ExitObjectType: ObjectType

---The `PLAYER` object-kind constant from the current runtime.
---@class PlayerObjectType: ObjectType

---Immutable namespace of typed native object kinds.
---
---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for
---unknown or non-string keys and attempted mutation.
---@class (exact) ObjectTypeNamespace
---@field ROOM RoomObjectType Detached room kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
---@field THING ThingObjectType Contained thing kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
---@field EXIT ExitObjectType Attached exit kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
---@field PLAYER PlayerObjectType Player kind; existing players may have this type, but scripts cannot create them.
---@see mux.error.codes.arg.invalid

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
---@field ANSI Flag Enables ANSI-capable output for the object.
---@field AUDIBLE Flag Enables sound-capable notifications associated with the object.
---@field AUDITORIUM Flag Applies auditorium-style speech propagation.
---@field BLIND Flag Marks the object as unable to see normally.
---@field CONNECTED Flag Marks a player as currently connected.
---@field DARK Flag Hides the object according to native visibility rules.
---@field FLOATING Flag Prevents ordinary location inheritance for the object.
---@field GAGGED Flag Prevents the object from speaking normally.
---@field GOING Flag Marks the object for deferred destruction.
---@field HALTED Flag Prevents queued command execution by the object.
---@field IN_CHARACTER Flag Marks the object as participating in in-character play.
---@field LIGHT Flag Makes the object visible through native light rules.
---@field MONITOR Flag Enables command monitoring behavior.
---@field NO_COMMAND Flag Prevents attributes on the object from acting as commands.
---@field SAFE Flag Protects the object from ordinary destruction.
---@field SUSPECT Flag Marks a player for suspect-activity monitoring.
---@field TRANSPARENT Flag Allows visibility through the object.
---@field WIZARD Flag Grants Wizard status under native privilege rules.
---@field XCODE Flag Marks an object as a BattleTech special object.
---@field ZOMBIE Flag Allows a thing to act through its owner under native rules.
---@see mux.error.codes.flag.invalid

---Dynamic, immutable lookup namespace for registered powers. Keys must use the
---canonical uppercase native name.
---
---Raises [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid)
---for unknown or non-string keys and attempted mutation.
---@class PowerNamespace
---@field IDLE Power Exempts a player from ordinary idle-timeout handling.
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

---Returns matching objects directly contained by or attached to this object.
---@param options? ContentsOptions Optional type and visibility filters.
---@return Object[] contents
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
function Object:contents(options) end

---Returns this exit's destination, or nil when it is unlinked or the
---destination is being destroyed.
---@return Object? destination
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
---during `@lua/check`, or
---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
---the receiver is not an exit or its stored destination is invalid.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:destination() end

---Sets this exit's destination, or clears it when `destination` is nil.
---@param destination DbRef|Object|nil Live object capable of containing objects, or nil to unlink this exit. This argument must be supplied explicitly.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `destination` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when the receiver is not an exit or the destination cannot contain objects, or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable) when the receiver or destination is being destroyed.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function Object:set_destination(destination) end

---Returns this thing or player's home, or nil when no home is assigned or the
---home is being destroyed.
---@return Object? home
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
---during `@lua/check`, or
---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
---the receiver is not a thing or player or its stored home is invalid.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:home() end

---Sets this thing or player's home to a live object capable of containing
---objects.
---@param new_home DbRef|Object Live room, thing, or player to assign as the object's home.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
---during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid)
---when `new_home` is omitted,
---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
---the receiver is not a thing or player, the home cannot contain objects, or
---the object would be its own home, or
---[`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable)
---when the receiver or home is being destroyed.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function Object:set_home(new_home) end

---Returns this thing or player's current location, or nil when no location is
---assigned or the location is being destroyed.
---@return Object? location
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
---during `@lua/check`, or
---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
---the receiver is not a thing or player or its stored location is invalid.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function Object:location() end

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

---A typed communication-channel flag constant from
---[`mux.comsys.flags`](lua://mux.comsys.flags). Its string form is the
---canonical uppercase name, and equality compares identity within the current
---Lua runtime.
---@class ChannelFlag

---Immutable namespace of supported communication-channel flags. Unknown or
---non-string lookups and attempted mutation raise
---[`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
---@class (exact) ChannelFlagNamespace
---@field PUBLIC ChannelFlag Makes the channel visible without a successful join lock.
---@field LOUD ChannelFlag Announces applicable connection and presence changes.
---@field TRANSPARENT ChannelFlag Relaxes hidden-member filtering in native channel displays.
---@see mux.error.codes.channel_flag.invalid

---Options for [`Channel:emit`](lua://Channel.emit).
---@class (exact) ChannelEmitOptions
---@field no_header? boolean Send the message without the usual `[channel]` prefix.

---Options for [`Channel:who`](lua://Channel.who).
---@class (exact) ChannelWhoOptions
---@field all? boolean Include inactive membership records.

---One communication-channel membership record.
---@class ChannelMember
---@field object Object Live member object.
---@field listening boolean Whether the member is currently listening to the channel.

---A generation-sensitive handle to one live communication channel. Handles
---remain stale after destruction even if a channel with the same name is
---created later. Assigning fields raises
---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@class Channel
---@field flags fun(self: Channel): ChannelFlags
---@field set_object fun(self: Channel, object?: DbRef|Object) Attaches the object supplying channel locks and description, or detaches it when omitted or nil.
---@see mux.error.codes.arg.invalid
local Channel = {}

---Returns the channel's exact name.
---@return string name
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
function Channel:name() end

---Returns the object that supplies the channel description and locks.
---@return Object? object The attached object, or nil when none is attached.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.object.invalid
function Channel:object() end

---Returns the number of channel membership records.
---@return integer count
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
function Channel:user_count() end

---Returns the channel's currently allocated membership capacity.
---@return integer count
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
function Channel:max_user_count() end

---Returns the channel's lifetime delivered-message count.
---@return integer count
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
function Channel:message_count() end

---Attaches an object that supplies channel locks and description, or detaches
---the current object when passed nil.
---@param object? DbRef|Object
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function Channel:set_object(object) end

---Opens the live administrative flag collection for this channel.
---@return ChannelFlags flags
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
function Channel:flags() end

---Emits an administrative channel message through native delivery, history,
---receive-lock, and message-count behavior.
---@param message string Valid UTF-8 without embedded NUL bytes.
---@param options? ChannelEmitOptions Unknown option fields are rejected.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.arg.invalid
function Channel:emit(message, options) end

---Returns channel membership records. By default the native active-member
---filter is applied; `options.all` includes inactive records.
---@param options? ChannelWhoOptions Unknown option fields are rejected.
---@return ChannelMember[] members
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.arg.invalid
function Channel:who(options) end

---Adds a player to this channel with a player-local command alias. The trusted
---operation bypasses the channel join lock. A quiet join suppresses only the
---channel-wide announcement; direct confirmations are still sent to the player.
---@param player DbRef|Object Player to add.
---@param alias string One to five printable ASCII characters without spaces.
---@param quiet boolean Whether to suppress the channel-wide join announcement.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.internal`](lua://mux.error.codes.internal).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.internal
function Channel:add_player(player, alias, quiet) end

---Announces a God-administered boot and removes a current member's channel
---aliases using the native side-effect path.
---@param object DbRef|Object Current channel member.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.object.invalid
function Channel:boot_player(object) end

---A live view of one channel's administrative flags. It becomes stale when
---its originating channel is destroyed.
---@class ChannelFlags
local ChannelFlags = {}

---Lists set flags in `PUBLIC`, `LOUD`, `TRANSPARENT` order.
---@return ChannelFlag[] flags
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
function ChannelFlags:list() end

---Tests whether the channel has a typed flag.
---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
---@return boolean present
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.channel_flag.invalid
function ChannelFlags:has(flag) end

---Sets a typed channel flag.
---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
---@return boolean changed Whether the flag changed from unset to set.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.channel_flag.invalid
function ChannelFlags:add(flag) end

---Clears a typed channel flag.
---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
---@return boolean changed Whether the flag changed from set to unset.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
---@see mux.error.codes.channel_flag.invalid
function ChannelFlags:remove(flag) end

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
---@field bold? boolean Whether to enable bold intensity.
---@field underline? boolean Whether to underline the text.
---@field inverse? boolean Whether to swap foreground and background presentation.

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
---Non-error causes are normalized to
---[`mux.error.codes.runtime`](lua://mux.error.codes.runtime). Raises
---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when the
---wrapper code is invalid.
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

---Trusted access to the live communication-channel registry. Mutations take
---effect immediately and are not rolled back when the surrounding Lua
---callback later fails.
---@class MuxComsysPackage
---@field flags ChannelFlagNamespace Immutable typed channel-flag constants.
local mux_comsys = {}

---Retrieves an existing communication channel by case-insensitive name.
---@param name string Existing channel name without embedded NUL bytes; the returned handle preserves canonical spelling.
---@return Channel channel
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.channel.invalid
function mux_comsys.channel(name) end

---Creates a private communication channel using the native channel-name
---rules. Names must be non-empty printable ASCII, contain no spaces, and be
---shorter than 50 bytes.
---@param name string New channel name.
---@return Channel channel
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid) when the name already exists.
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.channel.invalid
function mux_comsys.create_channel(name) end

---Permanently removes a live channel and its membership storage. The supplied
---handle and every flag handle derived from it become stale.
---@param channel Channel Live channel handle.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.channel.invalid
function mux_comsys.destroy_channel(channel) end

---Lists every live communication channel in case-insensitive name order, with
---original spelling used as the tie-breaker.
---@return Channel[] channels
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
---@see mux.error.codes.unavailable.checking
function mux_comsys.list_channels() end

---Fields accepted when creating a detached room through
---[`mux.world.create_object`](lua://mux.world.create_object).
---@class (exact) CreateRoomOptions
---@field type RoomObjectType The current runtime's [`mux.world.types.ROOM`](lua://mux.world.types.ROOM) constant.
---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.

---Fields accepted when creating and placing a thing through
---[`mux.world.create_object`](lua://mux.world.create_object).
---@class (exact) CreateThingOptions
---@field type ThingObjectType The current runtime's [`mux.world.types.THING`](lua://mux.world.types.THING) constant.
---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
---@field location DbRef|Object Required object that can contain the new thing.
---@field home? DbRef|Object Home object; defaults to `location` when omitted.
---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.

---Fields accepted when creating and attaching an exit through
---[`mux.world.create_object`](lua://mux.world.create_object).
---@class (exact) CreateExitOptions
---@field type ExitObjectType The current runtime's [`mux.world.types.EXIT`](lua://mux.world.types.EXIT) constant.
---@field name string Required UTF-8 name, optionally containing valid styled-text markup.
---@field location DbRef|Object Required source object capable of holding exits.
---@field destination? DbRef|Object Optional destination capable of containing objects; omission leaves the exit unlinked.
---@field zone? DbRef|Object Live thing or room to assign; omission preserves the native creator's inherited zone.

---Exact fields accepted by [`mux.world.create_object`](lua://mux.world.create_object).
---The selected typed object-kind constant determines which other fields apply.
---@alias CreateObjectOptions CreateRoomOptions|CreateThingOptions|CreateExitOptions

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

---Filters for [`Object:contents`](lua://Object.contents).
---@class (exact) ContentsOptions
---@field types? ObjectType[] Native object kinds to include; an empty array matches nothing.
---@field visible_to? DbRef|Object Viewer whose native visibility rules are applied.

---Filters for [`mux.world.list_objects`](lua://mux.world.list_objects).
---@class (exact) ListObjectsOptions
---@field types? ObjectType[] Native object kinds to include; an empty array matches nothing.
---@field in_zone? DbRef|Object Include only objects directly assigned to this zone.

---World database object access.
---@class MuxWorldPackage
---@field access AccessNamespace Immutable namespace of command-access constants.
---@field flags FlagNamespace Immutable namespace of registered flag constants.
---@field locks LockNamespace Immutable namespace of native lock constants.
---@field powers PowerNamespace Immutable namespace of registered power constants.
---@field types ObjectTypeNamespace Immutable namespace of native object-kind constants.
local mux_world = {}

---Creates a validated object handle from a dbref or existing handle.
---@param dbref DbRef|Object
---@return Object object
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function mux_world.object(dbref) end

---Lists database objects matching optional type and direct-zone filters.
---@param options? ListObjectsOptions Optional filters; unknown fields are rejected.
---@return Object[] objects Matching objects in ascending dbref order.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function mux_world.list_objects(options) end

---Creates a room, thing, or exit selected by a typed object-kind constant.
---Rooms are detached; things require a container and receive a home; exits
---require a source and may be linked to a destination. Unknown fields and
---fields that do not apply to the selected type are rejected.
---@param options CreateObjectOptions Exact creation fields selected by `options.type`.
---@return Object object Newly created object.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function mux_world.create_object(options) end

---Teleports a thing or player through the native movement path.
---@param options TeleportOptions Teleport fields; unknown fields are rejected.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.arg.invalid
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function mux_world.teleport_object(options) end

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
function mux_world.destroy_object(object, options) end

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
---@field comsys MuxComsysPackage Trusted live communication-channel administration.
---@field config MuxConfigPackage Read-only scalar server configuration.
---@field error MuxErrorPackage Structured errors and checked code nodes.
---@field session MuxSessionPackage Live connections and interactive flows.
---@field telnet MuxTelnetPackage Telnet protocol state and capabilities.
---@field text MuxTextPackage Styled-text utilities.
---@field world MuxWorldPackage World database object access.
mux = {}

---Checks the database for inconsistencies and repairs damage found by the
---default native `@dbck` pass. Findings are written to the server log.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
---@see mux.error.codes.unavailable.checking
function mux.check_db() end

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
mux.comsys = mux_comsys
mux.error = mux_error
mux.session = mux_session
mux.telnet = mux_telnet
mux.text = mux_text
mux.world = mux_world

return mux
