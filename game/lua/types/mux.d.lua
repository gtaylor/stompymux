---@meta _

---Maintained by `just update-lua-types`; edit the native bindings and their
---Doxygen comments, then refresh this definition instead of editing it alone.

---@alias DbRef integer Database object reference.
---@alias StateValue string|boolean|number
---@alias TelnetEnvironmentKind "var"|"uservar"
---@alias ObjectType "room"|"thing"|"exit"|"player"
---@alias NativeErrorRoot "mux"|"btech"|"testing"

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
---@class MuxConnectionErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.connection.invalid`.
---@field unavailable ErrorCode `mux.connection.unavailable`.
---@class MuxTextErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.text.invalid`.
---@class MuxModuleErrorCodes: ErrorCode
---@field invalid ErrorCode `mux.module.invalid`.
---@field unavailable ErrorCode `mux.module.unavailable`.
---@class MuxErrorCodes: ErrorCode
---@field arg MuxArgErrorCodes
---@field unavailable MuxUnavailableErrorCodes
---@field runtime ErrorCode `mux.runtime`.
---@field state MuxStateErrorCodes
---@field object MuxObjectErrorCodes
---@field attribute MuxAttributeErrorCodes
---@field connection MuxConnectionErrorCodes
---@field text MuxTextErrorCodes
---@field module MuxModuleErrorCodes
---@field internal ErrorCode `mux.internal`.

---@class ErrorCodeTree: ErrorCode
---@field [string] ErrorCodeTree

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
---@field dbref DbRef Read-only native database reference.
---@field name string Read-only current object name.
---@field type? ObjectType Read-only type, or nil for an unrecognized native object type.
---@field description? string Read-only native description.
---@field inside_description? string Read-only native inside description.
local Object = {}

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

---Tests whether an enactor passes this exit's default traversal lock.
---@param enactor DbRef|Object
---@return boolean passes
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
---@see mux.error.codes.object.unavailable
function Object:enter_lock_passes(enactor) end

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
function Object:attribute() end

---@class Connection
---@field object Object Connected player.
---@field name string Current object name.
---@field connected_for integer Connected duration in seconds.
---@field idle_for integer Idle duration in seconds.

---@class WhoSummary
---@field hidden integer Hidden-player count; currently always zero for this non-privileged view.
---@field record integer Record simultaneous-player count.
---@field maximum? integer Configured limit, or nil when unlimited.

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
---
---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
---@see mux.error.codes.arg.invalid
function mux_error.code_tree(root) end

---The native MUX host API.
---@class MuxPackage
---@field error MuxErrorPackage
mux = {}

---Creates a validated object handle from a dbref or existing handle.
---@param dbref DbRef|Object
---@return Object object
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.object.invalid
function mux.object(dbref) end

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
function mux.notify(object, message) end

---Lists connected players visible to the ordinary WHO command.
---@return Connection[] players
function mux.connected_players() end

---Returns the non-privileged WHO summary.
---@return WhoSummary summary
function mux.who_summary() end

---Tests whether a binary-safe RFC 1572 NEW-ENVIRON variable is defined.
---@param descriptor integer
---@param kind TelnetEnvironmentKind
---@param name string
---@return boolean defined
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.connection.invalid
function mux.telnet_environment_has(descriptor, kind, name) end

---Gets a binary-safe RFC 1572 NEW-ENVIRON value.
---@param descriptor integer
---@param kind TelnetEnvironmentKind
---@param name string
---@return string? value
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.connection.invalid
function mux.telnet_environment_get(descriptor, kind, name) end

---Attaches an interactive flow to a descriptor and displays its first prompt.
---@param descriptor integer
---@param module string Require-style module path.
---@param first_step string Key in the module's `flows` table.
---
---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.unavailable`](lua://mux.error.codes.connection.unavailable).
---@see mux.error.codes.unavailable.checking
---@see mux.error.codes.connection.unavailable
function mux.flow_start(descriptor, module, first_step) end

---Validates styled-text markup and returns it unchanged.
---@param value string
---@return string markup
---
---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
---@see mux.error.codes.text.invalid
function mux.markup(value) end

---Tests whether every byte is printable ASCII (0x20 through 0x7e).
---@param value string
---@return boolean printable
function mux.is_printable_ascii(value) end

---Wraps text in markup described by the supplied style options.
---@param value string
---@param options StyleOptions
---@return string styled
---
---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
---@see mux.error.codes.text.invalid
function mux.style(value, options) end

---Removes styled-text markup and ANSI styling.
---@param value string
---@return string plain
function mux.strip_style(value) end

---Measures visible byte width, excluding markup and ANSI styling.
---@param value string
---@return number width
function mux.text_width(value) end

---Safely truncates styled text to a non-negative visible byte width.
---@param value string
---@param width number
---@return string truncated
---
---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
---@see mux.error.codes.text.invalid
function mux.truncate_text(value, width) end

mux.error = mux_error

return mux
