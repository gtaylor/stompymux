---@meta _

---Maintained by `just update-lua-types`; edit the native bindings and their
---Doxygen comments, then refresh this definition instead of editing it alone.

---@alias BtechArgument string|number|boolean Scalar accepted by the legacy BattleTech function bridge.
---@alias BtechListItem string|number Item returned by a legacy BattleTech list result.
---@alias CharacterRef integer|string Character dbref or legacy character reference.
---@alias CharacterListKind "skills"|"advantages"|"attributes" Character-value category accepted by [`btech.character.list`](lua://btech.character.list).
---@alias CharacterValueMode 0|1|2|3|4 Legacy character-value lookup or mutation mode.
---@alias CriticalNameType 0|1 Critical-slot naming mode: template names or repair-part names.
---@alias PartCategory "ammo"|"weapon"|"weapons"|"weap"|"bomb"|"bombs"|"special"|"specials"|"cargo"|"carg"|"part"|"parts" Canonical or legacy part-category spelling.
---@alias PartNameSize "short"|"long"|"vlong" Requested native part-name length.
---@alias WeaponStat "VRT"|"TYPE"|"HEAT"|"DAMAGE"|"MIN"|"SR"|"MR"|"LR"|"CRIT"|"AMMO"|"WEIGHT"|"BV" Weapon statistic recognized by [`btech.parts.weapon_stat`](lua://btech.parts.weapon_stat).

---Checked native BattleTech error-code tree.
---@class BtechErrorCodes: ErrorCode
---@field unavailable ErrorCode `btech.unavailable`, raised during `@lua/check`.
---@field failed ErrorCode `btech.failed`, raised when a mapped legacy handler reports an error.

---@class BtechErrorPackage
---@field codes BtechErrorCodes

---Live units, templates, combat values, and status.
---@class BtechUnitPackage
local btech_unit = {}

---Battle maps, geometry, line of sight, and map messaging.
---@class BtechMapPackage
local btech_map = {}

---Part catalogues, installed parts, stores, and costs.
---@class BtechPartsPackage
local btech_parts = {}

---Character values, skills, experience, and piloting rolls.
---@class BtechCharacterPackage
local btech_character = {}

---Damage and technician-status queries.
---@class BtechRepairPackage
local btech_repair = {}

---Special-object fields and server-wide BattleTech queries.
---@class BtechSystemPackage
local btech_system = {}

---The native BattleTech host API. All functions are unavailable during `@lua/check`.
---@class BtechPackage
---@field unit BtechUnitPackage
---@field map BtechMapPackage
---@field parts BtechPartsPackage
---@field character BtechCharacterPackage
---@field repair BtechRepairPackage
---@field system BtechSystemPackage
---@field error BtechErrorPackage
btech = {}

---Adds a quantity of a part to an object's stores.
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@param target integer Stores-bearing object dbref.
---@param part_name string Recognized part name.
---@param quantity number Quantity to add, subject to the server cap.
---@return boolean success
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.add_stores(target, part_name, quantity) end

---Returns serialized armor values for a live unit section.
---@param unit integer
---@param section string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.armor_status(unit, section) end

---Returns serialized armor values for a unit-template section.
---@param reference string
---@param section string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.armor_status_ref(reference, section) end

---Lists character-value names in a category, optionally filtered by learned values.
---@param kind CharacterListKind
---@param character? CharacterRef
---@return BtechListItem[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_character.list(kind, character) end

---Describes one critical slot on a live unit.
---@param unit integer
---@param section string
---@param slot integer
---@param name_type? CriticalNameType `0` uses template names; `1` uses repair-part names.
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.crit_slot(unit, section, slot, name_type) end

---Describes one critical slot in a unit template.
---@param reference string
---@param section string
---@param slot integer
---@param name_type? CriticalNameType
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.crit_slot_ref(reference, section, slot, name_type) end

---Returns serialized status for one live-unit section.
---@param unit integer
---@param section string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.section_status(unit, section) end

---Returns serialized critical-slot status for one live-unit section.
---@param unit integer
---@param section string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.crit_status(unit, section) end

---Returns serialized critical-slot status for one template section.
---@param reference string
---@param section string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.crit_status_ref(reference, section) end

---Applies clustered damage and associated messages to a live unit.
---@param unit integer
---@param damage number Total damage from 1 through 1000.
---@param cluster_size number Damage per cluster, at least 1.
---@param direction number Legacy attack-direction code.
---@param force_critical boolean|number
---@param unit_message string
---@param los_message string
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.damage(unit, damage, cluster_size, direction, force_critical, unit_message, los_message) end

---Returns the formatted repair-job description for a live unit.
---@param unit integer
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_repair.damages(unit) end

---Tests whether a unit-template reference exists.
---@param reference string
---@return boolean exists
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_system.design_exists(reference) end

---Returns a live unit's engine rating.
---@param unit integer
---@return number rating
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.engine_rating(unit) end

---Returns a unit template's engine rating.
---@param reference string
---@return number rating
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.engine_rating_ref(reference) end

---Calculates a unit template's FASA base cost.
---@param reference string
---@return number cost
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.fasa_base_cost_ref(reference) end

---Calculates a live unit's battle value.
---@param unit integer
---@return number value
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.battle_value(unit) end

---Calculates a unit template's battle value.
---@param reference string
---@return number value
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.battle_value_ref(reference) end

---Calculates a unit template's second-generation battle value.
---@param reference string
---@return number value
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.battle_value2_ref(reference) end

---Calculates a unit template's defensive battle-value component.
---@param reference string
---@return number value
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.defensive_battle_value_ref(reference) end

---Calculates a unit template's offensive battle-value component.
---@param reference string
---@return number value
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.offensive_battle_value_ref(reference) end

---Gets a character attribute, skill level, target, experience, or threshold.
---@param character CharacterRef
---@param value integer|string Character-value code or name.
---@param mode CharacterValueMode
---@return number result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_character.value(character, value, mode) end

---Returns the configured cost of a recognized part.
---@param part_name string
---@return number cost
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.cost(part_name) end

---Calculates three-dimensional range between two units on a map.
---@param map integer
---@param unit_a integer
---@param unit_b integer
---@return number range
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.range(map, unit_a, unit_b) end

---Returns a live unit's effective maximum speed.
---@param unit integer
---@return number speed
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.real_max_speed(unit) end

---Returns a recognized part's weight in tons.
---@param part_name string
---@return number tons
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.weight(part_name) end

---Reads a script-visible native field from a live special object.
---@param object integer
---@param name string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_system.xcode_value(object, name) end

---Reads a script-visible native field from a unit template.
---@param reference string
---@param name string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_system.xcode_value_ref(reference, name) end

---Broadcasts a non-empty message from one map hex.
---@param map integer
---@param x number
---@param y number
---@param message string
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.hex_emit(map, x, y, message) end

---Tests whether a map hex lies in a configured blast zone.
---@param map integer
---@param x number
---@param y number
---@return boolean inside
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.hex_in_blast_zone(map, x, y) end

---Tests a live unit's line of sight to a map hex.
---@param unit integer
---@param x number
---@param y number
---@return boolean visible
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.hex_line_of_sight(unit, x, y) end

---Resolves a two-character tactical ID on a unit's map.
---@param unit_or_map integer
---@param id string
---@return integer dbref
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.id_to_dbref(unit_or_map, id) end

---Returns the current BattleTech event lag.
---@return number lag
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_system.lag() end

---Lists blast-zone data as repeating x, y, and radius numbers.
---@param map integer
---@return number[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.blast_zones(map) end

---Loads a map file and clears units and map objects from the target map.
---@param map integer
---@param name string
---@param clear? boolean Ignored compatibility argument.
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.load(map, name, clear) end

---Loads a unit template into a live unit object.
---@param unit integer
---@param reference string
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.load(unit, reference) end

---Tests line of sight between two live units.
---@param unit integer
---@param target integer
---@return boolean visible
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.unit_line_of_sight(unit, target) end

---Makes a piloting roll and causes a fall when it fails.
---@param unit integer
---@param roll_modifier number
---@param damage_modifier number
---@return boolean result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.make_pilot_roll(unit, roll_modifier, damage_modifier) end

---Returns the elevation of a map hex.
---@param map integer
---@param x number
---@param y number
---@return number elevation
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.elevation(map, x, y) end

---Broadcasts a non-empty message to all or nearby units on a map.
---@param map integer
---@param message string
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.emit(map, message) end

---Returns the terrain code of a map hex.
---@param map integer
---@param x number
---@param y number
---@return string terrain
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.terrain(map, x, y) end

---Lists unit dbrefs on a map.
---@param map integer
---@return integer[] units
---@overload fun(map: integer, x: number, y: number, range: number): integer[]
---@overload fun(map: integer, x: number, y: number, z: number, range: number): integer[]
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.units(map) end

---Lists configured radio channels for a live unit.
---@param unit integer
---@return BtechListItem[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.frequencies(unit) end

---Returns the number of pending repair jobs on a live unit.
---@param unit integer
---@return number count
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_repair.job_count(unit) end

---Returns the broad category of a recognized part.
---@param part_name string
---@return string category
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.type(part_name) end

---Finds packed part IDs whose names match text.
---@param query string
---@return integer[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.match(query) end

---Returns a selected name for a packed part ID.
---@param part integer
---@param size PartNameSize
---@return string name
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.name(part, size) end

---Lists canonical part categories accepted by [`btech.parts.list`](lua://btech.parts.list).
---@return string[] categories
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.categories() end

---Lists canonical long part names in a category or accepted alias.
---@param category PartCategory
---@return string[] parts
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.list(category) end

---Returns the weapon and ammunition payload of a unit template.
---@param reference string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.payload_ref(reference) end

---Removes a quantity of a part from an object's stores.
---@param target integer
---@param part_name string
---@param quantity number
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.remove_stores(target, part_name, quantity) end

---Sets one armor-status field on a live-unit section.
---@param unit integer
---@param section string
---@param field string
---@param value number
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.set_armor_status(unit, section, field, value) end

---Sets a character value or adjusts skill experience.
---@param character CharacterRef
---@param value integer|string
---@param amount number
---@param mode number `0` sets value, `1` target, `3` XP; other nonzero modes add XP.
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_character.set_value(character, value, amount, mode) end

---Sets a live unit's maximum speed and corrects its current speed.
---@param unit integer
---@param speed number
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.set_max_speed(unit, speed) end

---Sets the non-negative configured cost of a recognized part.
---@param part_name string
---@param cost number
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.set_cost(part_name, cost) end

---Sets a live unit's tonnage and original weight.
---@param unit integer
---@param tons number
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.set_tons(unit, tons) end

---Writes a script-writable native field on a live special object.
---@param object integer
---@param name string
---@param value string|number
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_system.set_xcode_value(object, name, value) end

---Places a live unit on a map at specified coordinates.
---@param unit integer
---@param map integer
---@param x number
---@param y number
---@param z? number Defaults to zero.
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.set_xy(unit, map, x, y, z) end

---Sends a template's critical-status display to a player.
---@param reference string
---@param player integer
---@param section string
---@return string result Legacy renderer output, commonly empty.
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.show_crit_status_ref(reference, player, section) end

---Sends a unit template's status display to a player.
---@param reference string
---@param player integer
---@return string result Legacy renderer output, commonly empty.
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.show_status_ref(reference, player) end

---Sends a unit template's weapon-specification display to a player.
---@param reference string
---@param player integer
---@return string result Legacy renderer output, commonly empty.
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.show_weapon_specs_ref(reference, player) end

---Returns a quantity for one part, or lists stored parts when the part is omitted.
---@param target integer
---@param part_name string
---@return number[] result One-element quantity array.
---@overload fun(target: integer): BtechListItem[]
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.stores(target, part_name) end

---Returns a quantity or lists stored parts using short names.
---@param target integer
---@param part_name string
---@return number[] result One-element quantity array.
---@overload fun(target: integer): BtechListItem[]
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.stores_short(target, part_name) end

---Lists parts needed to repair a live unit.
---@param unit integer
---@return BtechListItem[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_repair.tech_list(unit) end

---Lists parts needed to repair a unit template.
---@param reference string
---@return BtechListItem[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_repair.tech_list_ref(reference) end

---Returns formatted repair status for a live unit.
---@param unit integer
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_repair.tech_status(unit) end

---Runs the legacy technician-time query.
---@return number value
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_repair.tech_time() end

---Returns the configured experience threshold for a skill.
---@param skill string
---@return number threshold
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_character.threshold(skill) end

---Lists weapons assigned to a unit's zero-based target-interlock circuit.
---@param unit integer
---@param tic integer
---@return BtechListItem[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.tic_weapons(unit, tic) end

---Tests whether a live unit has an active repair event.
---@param unit integer
---@return boolean under_repair
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_repair.under_repair(unit) end

---Tests whether a live unit can be repaired.
---@param unit integer
---@return boolean fixable
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_repair.unit_fixable(unit) end

---Lists parts installed on a live unit.
---@param unit integer
---@return BtechListItem[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.installed(unit) end

---Lists parts installed in a unit template.
---@param reference string
---@return BtechListItem[] values
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.installed_ref(reference) end

---Recursively updates links associated with a map.
---@param map integer
---@return boolean success
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_map.update_links(map) end

---Returns serialized weapon status for a live unit or optional section.
---@param unit integer
---@param section? string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.weapon_status(unit, section) end

---Returns serialized weapon status for a unit template or optional section.
---@param reference string
---@param section? string
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_unit.weapon_status_ref(reference, section) end

---Returns a selected numeric weapon-catalog statistic as serialized text.
---@param weapon string
---@param stat WeaponStat
---@return string result
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_parts.weapon_stat(weapon, stat) end

---Lists live unit dbrefs assigned to a zone.
---@param zone integer
---@return integer[] units
---
---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
---@see btech.error.codes.unavailable
---@see mux.error.codes.arg.invalid
---@see btech.error.codes.failed
function btech_system.zone_units(zone) end

return btech
