//! LuaLS contract blocks for the btech system surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00544
//|---Seconds of event lag accumulated by the running event queue.
//|---@return integer seconds
//|function btech_system.event_lag() end
// lua-types-end

// lua-types-begin btech 00545
//|---List registered BattleTech units contained in a zone.
//|---@param zone DbRef|Object
//|---@return Object[] units
//|function btech_system.units_in_zone(zone) end
// lua-types-end
