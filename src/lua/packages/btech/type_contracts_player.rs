//! LuaLS contract blocks for the btech player surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00354
//|---Read saved map dimensions or replace them; omitted fields in a replacement use standard defaults.
//|---Trusted callback code owns authorization to change the selected player's preferences.
//|---Native tactical/LRS and Lua tactical/LRS/viewport use these defaults; navigate stays radius two.
//|---@param player integer Live player dbref.
//|---@param dimensions BattleViewDimensions? Validated replacement; omit for a read-only query.
//|---@return BattleViewDimensions
//|function btech_player.view_dimensions(player, dimensions) end
// lua-types-end

// lua-types-begin btech 00357
//|---Read or replace saved contact-list inclusion policy for a live player.
//|---Trusted callback code owns edit authorization; omitted replacement fields use defaults.
//|---@param player integer
//|---@param preferences BattleContactPreferences?
//|---@return BattleContactPreferences
//|function btech_player.contact_preferences(player, preferences) end
// lua-types-end

// lua-types-begin btech 00359
//|---Decode transient unit-list options d/s/e/a/t and persistent exclusion prefix !.
//|---Does not access game state; b requests building identification in native output.
//|---@param options string Single word, up to fifty characters are processed.
//|---@param brief_buildings boolean? Initial building inclusion from unit brief settings; defaults false.
//|---@return BattleContactOptions
//|function btech_player.contact_options(options, brief_buildings) end
// lua-types-end

// lua-types-begin btech 00455
//|---Read the saved personal-combat loadout, or nil when none is configured.
//|---@param player DbRef|Object
//|---@return BattlePersonalCombatLoadout|nil loadout
//|function btech_player.loadout(player) end
// lua-types-end

// lua-types-begin btech 00456
//|---Replace the saved personal-combat loadout; nil clears it.
//|---@param player DbRef|Object
//|---@param loadout BattlePersonalCombatLoadout|nil
//|function btech_player.set_loadout(player, loadout) end
// lua-types-end

// lua-types-begin btech 00457
//|---Read the saved MechWarrior template reference, or nil when unset.
//|---@param player DbRef|Object
//|---@return string|nil reference
//|function btech_player.mechwarrior_template(player) end
// lua-types-end

// lua-types-begin btech 00458
//|---Replace the saved MechWarrior template reference; nil clears it.
//|---@param player DbRef|Object
//|---@param reference string|nil
//|function btech_player.set_mechwarrior_template(player, reference) end
// lua-types-end

// lua-types-begin btech 00459
//|---Read the saved tactical contact and display preferences.
//|---@param player DbRef|Object
//|---@return BattleUiPreferencesState preferences
//|function btech_player.ui_preferences(player) end
// lua-types-end

// lua-types-begin btech 00460
//|---Replace the saved tactical contact and display preferences; nil clears them.
//|---@param player DbRef|Object
//|---@param preferences BattleUiPreferencesState|nil
//|function btech_player.set_ui_preferences(player, preferences) end
// lua-types-end
