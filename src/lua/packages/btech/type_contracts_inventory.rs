//! LuaLS contract blocks for the btech inventory surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00227
//|---Wizard signed adjustment of one catalogue match, ordered by long name after exact matching.
//|---Positive counts cap at 50000; negative counts are not capped. Zero succeeds without matching.
//|---Stock, load correction and diagnostics roll back together. No match returns false.
//|---@param actor integer
//|---@param object integer
//|---@param pattern string Fewer than 2048 bytes.
//|---@param quantity integer Signed 32-bit count.
//|---@return boolean
//|function btech_inventory.add_stores(actor, object, pattern, quantity) end
// lua-types-end

// lua-types-begin btech 00228
//|---Wizard catalogue-based addition; amount is capped at 50000 per match.
//|---Wizards other than GOD may select at most 20 catalogue entries. Stock and diagnostics commit together.
//|---@param actor integer
//|---@param object integer
//|---@param pattern string Exact abbreviation/full name, then wildcard; may match absent stock.
//|---@param quantity integer Positive requested quantity per match.
//|---@return CargoRow[] Requested changes after the request cap.
//|function btech_inventory.add(actor, object, pattern, quantity) end
// lua-types-end

// lua-types-begin btech 00229
//|---Wizard removal floors stock at zero; reports and diagnostics retain the capped requested amount.
//|---@param actor integer
//|---@param object integer
//|---@param pattern string
//|---@param quantity integer Positive requested quantity per match.
//|---@return CargoRow[]
//|function btech_inventory.remove(actor, object, pattern, quantity) end
// lua-types-end

// lua-types-begin btech 00230
//|---Wizard reset removes every stock row and emits one reset record, including for an empty holder.
//|---@param actor integer
//|---@param object integer
//|function btech_inventory.clear(actor, object) end
// lua-types-end

// lua-types-begin btech 00232
//|---Wizard cleanup of loose stock, removing structural placeholders and unknown identifiers.
//|---Preserves installed equipment; reconciles carrying load. Callback failure restores inventory.
//|---@param actor integer
//|---@param object integer
//|---@return InventoryCleanup
//|function btech_inventory.fix(actor, object) end
// lua-types-end

// lua-types-begin btech 00233
//|---Describe stock by exact name or stored identifier; names ignore ASCII case.
//|---@param part string|integer
//|---@return Part
//|function btech_inventory.part(part) end
// lua-types-end

// lua-types-begin btech 00234
//|---Physical loose-stock mass in 1/1024 tons, before chassis cargo discounts.
//|---@param object integer
//|---@return integer
//|function btech_inventory.mass(object) end
// lua-types-end

// lua-types-begin btech 00235
//|---Wizard stock correction by exact part name, sharing validation and callback rollback with set.
//|---@param actor integer
//|---@param object integer
//|---@param name string
//|---@param quantity integer From zero through 2147483647.
//|function btech_inventory.set_named(actor, object, name, quantity) end
// lua-types-end

// lua-types-begin btech 00236
//|---Read an object's detached, ordered loose-parts stock in a callback.
//|---@param object integer
//|---@return InventoryEntry[]
//|function btech_inventory.read(object) end
// lua-types-end

// lua-types-begin btech 00237
//|---Wizard stock correction using stored identifiers; zero quantity removes the entry.
//|---Stock, immediate load correction and EconInfo diagnostics participate in callback rollback.
//|---Unchanged quantities emit no record. Does not install equipment or perform cargo loading.
//|---@param actor integer
//|---@param object integer
//|---@param part integer Nonnegative signed-32-bit identifier.
//|---@param quantity integer From zero through 2147483647.
//|function btech_inventory.set(actor, object, part, quantity) end
// lua-types-end

// lua-types-begin btech 00415
//|---Return all part forms in short-name order, without requiring live stock.
//|---@param actor integer Wizard requesting inspection.
//|---@return table[] forms Part ID, short_name, long_name and very_long_name.
//|function btech_inventory.forms(actor) end
// lua-types-end
