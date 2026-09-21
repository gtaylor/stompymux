//! LuaLS contract blocks for the mux telnet surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00133
//|---Gets a binary-safe RFC 1572 NEW-ENVIRON value.
//|---@param descriptor integer Live descriptor ID, normally `ctx.descriptor`.
//|---@param kind TelnetEnvironmentKind NEW-ENVIRON variable namespace.
//|---@param name string Binary-safe variable name.
//|---@return string? value Binary-safe value, or nil when the variable is absent.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.connection.invalid
//|function mux_telnet.environment_get(descriptor, kind, name) end
// lua-types-end

// lua-types-begin mux 00134
//|---Tests whether a binary-safe RFC 1572 NEW-ENVIRON variable is defined.
//|---@param descriptor integer Live descriptor ID, normally `ctx.descriptor`.
//|---@param kind TelnetEnvironmentKind NEW-ENVIRON variable namespace.
//|---@param name string Binary-safe variable name.
//|---@return boolean defined Whether the variable is present, including with an empty value.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.connection.invalid
//|function mux_telnet.environment_has(descriptor, kind, name) end
// lua-types-end
