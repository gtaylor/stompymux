//! LuaLS contract blocks for the btech cargo surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00240
//|---Read detached stock in the actor's current location. Requires cargo commands enabled.
//|---@param actor integer
//|---@param pattern? string Case-insensitive stock name, numeric part ID, or wildcard pattern.
//|---@return CargoRow[]
//|function btech_cargo.manifest(actor, pattern) end
// lua-types-end

// lua-types-begin btech 00241
//|---Read hangar stock from a running unit at the configured loading point.
//|---@param actor integer
//|---@param pattern? string
//|---@return CargoRow[]
//|function btech_cargo.stores(actor, pattern) end
// lua-types-end

// lua-types-begin btech 00242
//|---Load matching hangar stock into a stationary, running CargoTech unit.
//|---Exact abbreviations precede exact catalogue names, then wildcard names; selection is independent of available stock.
//|---Transfers, throttle correction and economy log records are atomic and participate in callback rollback.
//|---@param actor integer
//|---@param pattern string
//|---@param quantity integer Positive request per matched row, capped at 50000 and available stock.
//|---@return CargoRow[] Transferred quantities.
//|function btech_cargo.load(actor, pattern, quantity) end
// lua-types-end

// lua-types-begin btech 00243
//|---Unload matching CargoTech stock onto the current map; startup and loading-point checks do not apply.
//|---@param actor integer
//|---@param pattern string
//|---@param quantity integer Positive request per matched row, capped at 50000 and available stock.
//|---@return CargoRow[] Transferred quantities.
//|function btech_cargo.unload(actor, pattern, quantity) end
// lua-types-end
