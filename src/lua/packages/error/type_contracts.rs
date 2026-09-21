//! LuaLS contract blocks for the mux error surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00074
//|---Tests this error's code using dotted-prefix matching.
//|---@param code string|ErrorCode
//|---@return boolean matches
//|function Error:is(code) end
// lua-types-end

// lua-types-begin mux 00075
//|---Returns the deepest table-valued cause, or this error when it has none.
//|---@return any root
//|function Error:root() end
// lua-types-end

// lua-types-begin mux 00121
//|---Returns a truthy value unchanged or raises `err` unchanged.
//|---@generic T
//|---@param value T
//|---@param err any
//|---@return T value
//|---
//|---Raises `err` unchanged when `value` is false or nil.
//|function mux_error.check(value, err) end
// lua-types-end

// lua-types-begin mux 00122
//|---Returns the cached checked native code tree for a root.
//|---@param root NativeErrorRoot
//|---@return ErrorCodeTree codes
//|---@overload fun(root: "mux"): MuxErrorCodes
//|---@overload fun(root: "btech"): BtechErrorCodes
//|---@overload fun(root: "testing"): TestingErrorCodes
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
//|---@see mux.error.codes.arg.invalid
//|function mux_error.code_tree(root) end
// lua-types-end

// lua-types-begin mux 00123
//|---Tests a table's code using exact or dotted-prefix matching.
//|---@param value any
//|---@param code string|ErrorCode
//|---@return boolean matches
//|---
//|---Raises an ordinary Lua type error when `code` cannot be converted to a string.
//|function mux_error.is(value, code) end
// lua-types-end

// lua-types-begin mux 00124
//|---Builds a checked code-symbol tree for an author-defined namespace.
//|---@param prefix string
//|---@param names string[]
//|---@return ErrorCodeTree codes
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
//|---@see mux.error.codes.arg.invalid
//|function mux_error.namespace(prefix, names) end
// lua-types-end

// lua-types-begin mux 00125
//|---Creates a structured error without raising it.
//|---@param fields ErrorFields
//|---@return Error error
//|---
//|function mux_error.new(fields) end
// lua-types-end

// lua-types-begin mux 00126
//|---Calls a function, returning all results on success or a normalized traced error.
//|---@generic R...
//|---@param fn fun(...): R...
//|---@param ... any
//|---@return true, R...
//|---@overload fun(fn: function, ...: any): false, Error|CaughtError
//|function mux_error.pcall(fn, ...) end
// lua-types-end

// lua-types-begin mux 00127
//|---Raises a structured error with the requested code.
//|---@param code string|ErrorCode
//|---@param message string
//|---@param detail? any
//|---
//|---Raises the requested code. Ordinary Lua type errors are raised when `code`
//|---or `message` cannot be converted to a string.
//|function mux_error.raise(code, message, detail) end
// lua-types-end

// lua-types-begin mux 00128
//|---Wraps a failure as the cause of a new structured error.
//|---@param err any
//|---@param code string|ErrorCode
//|---@param message string
//|---@return Error error
//|---
//|---Non-error causes are normalized to
//|---[`mux.error.codes.runtime`](lua://mux.error.codes.runtime). Ordinary Lua type
//|---errors are raised when `code` or `message` cannot be converted to a string.
//|---@see mux.error.codes.runtime
//|function mux_error.wrap(err, code, message) end
// lua-types-end
