//! LuaLS contract blocks for the btech repair surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00569
//|---Apply one immediate repair with operation from btech.repair.operations.
//|---@param unit DbRef|Object
//|---@param repair BattleImmediateRepair
//|function btech_repair.apply(unit, repair) end
// lua-types-end

// lua-types-begin btech 00570
//|---Report whether no original nonexempt section is destroyed; Mechs exempt all but the
//|---center torso, ground vehicles exempt the turret and VTOLs exempt the rotor.
//|---@param unit DbRef|Object
//|---@return boolean fixable
//|function btech_repair.is_fixable(unit) end
// lua-types-end

// lua-types-begin btech 00571
//|---Seconds until the player's configured technician becomes available.
//|---@param player DbRef|Object
//|---@return integer seconds
//|function btech_repair.technician_available_in(player) end
// lua-types-end
