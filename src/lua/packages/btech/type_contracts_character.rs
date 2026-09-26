//! LuaLS contract blocks for the btech character surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00090
//|---Return the detached catalog of supported advantages; gameplay availability varies by action.
//|---@return BattleAdvantageDefinition[]
//|function btech_character.advantages() end
// lua-types-end

// lua-types-begin btech 00092
//|---Return a detached catalog in canonical lookup order.
//|---@return BattleSkillDefinition[]
//|function btech_character.skills() end
// lua-types-end

// lua-types-begin btech 00093
//|---List canonical names in catalog order. An optional player filters skills with nonzero value or XP.
//|---Advantages and attributes remain complete when a player is supplied. Requires a callback.
//|---@param kind "skills"|"advantages"|"attributes" Full category names are case-insensitive; abbreviations are rejected.
//|---@param player? integer|string Live player id, name, account alias or #dbref; omit rather than passing explicit nil.
//|---@return string[]
//|function btech_character.list(kind, player) end
// lua-types-end

// lua-types-begin btech 00095
//|---Inspect progress without changing XP. Requires a callback and existing character attributes.
//|---@param player integer
//|---@param skill string Canonical name or short alias.
//|---@return BattleSkillProgress
//|function btech_character.progress(player, skill) end
// lua-types-end

// lua-types-begin btech 00096
//|---Current runtime XP threshold; fails for unknown skills. Requires a callback.
//|---@param skill string Canonical name or short alias.
//|---@return integer
//|function btech_character.threshold(skill) end
// lua-types-end

// lua-types-begin btech 00097
//|---Set a runtime XP threshold as a wizard. Defaults return after database reload.
//|---Existing earned levels are recalculated on the next accepted award.
//|---@param player integer Wizard actor.
//|---@param skill string Canonical name or short alias.
//|---@param threshold integer From 0 through 2147483647; zero disables earned levels.
//|---@return boolean
//|function btech_character.set_threshold(player, skill, threshold) end
// lua-types-end

// lua-types-begin btech 00098
//|---Detached saved character attributes and health; fails when no profile exists.
//|---@param player integer
//|---@return BattleCharacter
//|function btech_character.state(player) end
// lua-types-end

// lua-types-begin btech 00417
//|---Publish a wizard-only skill leaderboard without changing XP.
//|---@param actor integer
//|---@param skill string Canonical skill name or alias.
//|---@return table report Skill, counted players, total balance, top sixteen entries and literal text.
//|function btech_character.xptop(actor, skill) end
// lua-types-end

// lua-types-begin btech 00475
//|---Add signed skill experience using the shared C range semantics.
//|---@param character DbRef|Object Player object.
//|---@param skill string Canonical name or alias.
//|---@param amount integer
//|function btech_character.add_skill_experience(character, skill, amount) end
// lua-types-end

// lua-types-begin btech 00476
//|---Return ordered value definitions of one kind; a supplied player filters unsaved
//|---skills and advantages while attributes stay complete.
//|---@param kind string Char_value, Char_skill, Char_advantage or Char_attribute.
//|---@param character? DbRef|Object
//|---@return BattleCharacterValueDefinition[] definitions
//|function btech_character.catalog(kind, character) end
// lua-types-end

// lua-types-begin btech 00477
//|---Read the configured runtime experience threshold of one skill.
//|---@param skill string Canonical name or alias.
//|---@return integer threshold
//|function btech_character.experience_threshold(skill) end
// lua-types-end

// lua-types-begin btech 00478
//|---Replace the stored unsigned 32-bit skill experience.
//|---@param character DbRef|Object Player object.
//|---@param skill string
//|---@param experience integer
//|function btech_character.set_skill_experience(character, skill, experience) end
// lua-types-end

// lua-types-begin btech 00479
//|---Set the raw skill amount needed for the requested target; rejects non-skills and
//|---unreachable targets.
//|---@param character DbRef|Object Player object.
//|---@param skill string
//|---@param target integer
//|function btech_character.set_skill_target(character, skill, target) end
// lua-types-end

// lua-types-begin btech 00480
//|---Set one character value by name or code, preserving the C unsigned-byte storage.
//|---@param character DbRef|Object Player object.
//|---@param value string|integer Character-value name or code.
//|---@param amount integer
//|function btech_character.set_value(character, value, amount) end
// lua-types-end

// lua-types-begin btech 00481
//|---Read one character value; skills additionally report target and experience progress.
//|---@param character DbRef|Object Player object.
//|---@param value string|integer Character-value name, prefix or code.
//|---@return BattleCharacterValueReport result
//|function btech_character.value(character, value) end
// lua-types-end
