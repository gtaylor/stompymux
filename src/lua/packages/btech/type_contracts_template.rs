//! LuaLS contract blocks for the btech template surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00020
//|---Read a biped asset without instantiating or activating a unit.
//|---@param name string Relative name under database.mech_database.
//|---@return BattleTemplate
//|function btech_template.inspect(name) end
// lua-types-end

// lua-types-begin btech 00021
//|---Resolve supported equipment; does not validate chassis construction or enable simulation.
//|---@param name string Relative name under database.mech_database.
//|---@return BattleLoadout
//|function btech_template.loadout(name) end
// lua-types-end

// lua-types-begin btech 00255
//|---Preview construction without registering a unit or modifying the source asset.
//|---@param name string Bounded asset name from the configured mech directory.
//|---@return BattleTemplateCheck
//|function btech_template.check(name) end
// lua-types-end

// lua-types-begin btech 00495
//|---Read current, original and rear armor values; an omitted section reports the totals.
//|---@param reference string Relative name under database.mech_database.
//|---@param section? BattleSection Typed section constant from btech.unit.sections.
//|---@return BattleArmorStatus status
//|function btech_template.armor(reference, section) end
// lua-types-end

// lua-types-begin btech 00496
//|---Read the constructed base cost in C-bills.
//|---@param reference string
//|---@return integer cost
//|function btech_template.base_cost(reference) end
// lua-types-end

// lua-types-begin btech 00497
//|---Read offensive, defensive and total Battle Value.
//|---@param reference string
//|---@return BattleBattleValue value
//|function btech_template.battle_value(reference) end
// lua-types-end

// lua-types-begin btech 00498
//|---List one section's critical slots with resolved parts, modes and ammunition state.
//|---@param reference string
//|---@param section BattleSection Typed section constant from btech.unit.sections.
//|---@return BattleCriticalSlot[] slots
//|function btech_template.critical_slots(reference, section) end
// lua-types-end

// lua-types-begin btech 00499
//|---Read the engine rating and suspension factor.
//|---@param reference string
//|---@return BattleEngine engine
//|function btech_template.engine(reference) end
// lua-types-end

// lua-types-begin btech 00500
//|---Report whether the reference resolves to a loadable template.
//|---@param reference string
//|---@return boolean exists
//|function btech_template.exists(reference) end
// lua-types-end

// lua-types-begin btech 00501
//|---List installed equipment in catalogue order.
//|---@param reference string
//|---@return BattlePartStack[] parts
//|function btech_template.installed_parts(reference) end
// lua-types-end

// lua-types-begin btech 00502
//|---List carried ammunition stock in catalogue order.
//|---@param reference string
//|---@return BattlePartStack[] parts
//|function btech_template.payload(reference) end
// lua-types-end

// lua-types-begin btech 00503
//|---Publish the full template status report to a player.
//|---@param reference string
//|---@param player DbRef|Object
//|function btech_template.show_status(reference, player) end
// lua-types-end

// lua-types-begin btech 00504
//|---Publish the template weapon specifications to a player.
//|---@param reference string
//|---@param player DbRef|Object
//|function btech_template.show_weapon_specs(reference, player) end
// lua-types-end

// lua-types-begin btech 00505
//|---Publish one section's critical status report to a player.
//|---@param reference string
//|---@param player DbRef|Object
//|---@param section BattleSection Typed section constant from btech.unit.sections.
//|function btech_template.show_critical_status(reference, player, section) end
// lua-types-end

// lua-types-begin btech 00506
//|---List configured and inferred technologies.
//|---@param reference string
//|---@return BattleTechnology[] technologies
//|function btech_template.technologies(reference) end
// lua-types-end

// lua-types-begin btech 00507
//|---List mounted weapons in mounting order; an optional section restricts the result.
//|---@param reference string
//|---@param section? BattleSection Typed section constant from btech.unit.sections.
//|---@return BattleMountedWeapon[] weapons
//|function btech_template.weapons(reference, section) end
// lua-types-end
