//! LuaLS contract blocks for the mux config surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00120
//|---Returns the live scalar value of an exact, case-sensitive configuration directive.
//|---@param name string Configuration directive name; embedded NUL bytes are rejected.
//|---@return ConfigValue value Current value represented by its native Lua scalar type.
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.config.not_found`](lua://mux.error.codes.config.not_found), [`mux.error.codes.config.unsupported`](lua://mux.error.codes.config.unsupported), or [`mux.error.codes.internal`](lua://mux.error.codes.internal).
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.config.not_found
//|---@see mux.error.codes.config.unsupported
//|---@see mux.error.codes.internal
//|function mux_config.get(name) end
// lua-types-end
