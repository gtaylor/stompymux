//! LuaLS contract blocks for the btech weapon surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00222
//|---Read detached effective values using a canonical or manufacturer-qualified weapon name.
//|---Examples: IS.MediumLaser or Magna.IS.MediumLaser; exact names ignore ASCII case.
//|---@param name string
//|---@return BattleWeaponValues
//|function btech_weapon.settings(name) end
// lua-types-end

// lua-types-begin btech 00223
//|---Wizard-only runtime override. Existing countdowns are unchanged; restart restores catalogue defaults.
//|---@param actor integer
//|---@param name string
//|---@param seconds integer From 1 through 127.
//|---@return BattleWeaponValues
//|function btech_weapon.set_recycle(actor, name, seconds) end
// lua-types-end

// lua-types-begin btech 00224
//|---Wizard-only runtime override used by valuation and Battle Value XP. Restart restores catalogue defaults.
//|---@param actor integer
//|---@param name string
//|---@param value integer From 0 through 2147483647.
//|---@return BattleWeaponValues
//|function btech_weapon.set_battle_value(actor, name, value) end
// lua-types-end
