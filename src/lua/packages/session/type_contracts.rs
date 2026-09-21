//! LuaLS contract blocks for the mux session surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00130
//|---Lists connected players visible to the ordinary WHO command.
//|---@return Connection[] players
//|function mux_session.connected_players() end
// lua-types-end

// lua-types-begin mux 00131
//|---Attaches an interactive flow to a descriptor and displays its first prompt.
//|---@param descriptor integer
//|---@param module string Require-style module path.
//|---@param first_step string Key in the module's `flows` table.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid), [`mux.error.codes.connection.unavailable`](lua://mux.error.codes.connection.unavailable), or [`mux.error.codes.module.invalid`](lua://mux.error.codes.module.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.connection.invalid
//|---@see mux.error.codes.connection.unavailable
//|---@see mux.error.codes.module.invalid
//|function mux_session.flow_start(descriptor, module, first_step) end
// lua-types-end

// lua-types-begin mux 00132
//|---Returns the non-privileged WHO summary.
//|---@return WhoSummary summary
//|function mux_session.who_summary() end
// lua-types-end
