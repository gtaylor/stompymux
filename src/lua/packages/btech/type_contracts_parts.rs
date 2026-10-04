//! LuaLS contract blocks for the btech parts surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00524
//|---Apply one signed atomic stock edit; a nonzero integral delta is required.
//|---@param target DbRef|Object Live object holding stock.
//|---@param part BattlePartRef
//|---@param delta integer
//|function btech_parts.adjust_stores(target, part, delta) end
// lua-types-end

// lua-types-begin btech 00525
//|---Return the six detached part categories in canonical order.
//|---@return BattlePartCategory[] categories
//|function btech_parts.categories() end
// lua-types-end

// lua-types-begin btech 00526
//|---List registered parts in catalogue order; a case-insensitive category filters them.
//|---@param category? string
//|---@return BattlePartDefinition[] parts
//|function btech_parts.list(category) end
// lua-types-end

// lua-types-begin btech 00527
//|---Resolve one registered part by ID, case-insensitive name or {id} record.
//|---@param part BattlePartRef
//|---@return BattlePartDefinition|nil part
//|function btech_parts.resolve(part) end
// lua-types-end

// lua-types-begin btech 00528
//|---Search names with *, ? and backslash-escaped quick-wild matching.
//|---@param query string Nonempty query.
//|---@return BattlePartDefinition[] parts
//|function btech_parts.search(query) end
// lua-types-end

// lua-types-begin btech 00529
//|---Set the cost of one registered part.
//|---@param part BattlePartRef
//|---@param cost integer From 0 through 2^53-1.
//|function btech_parts.set_cost(part, cost) end
// lua-types-end

// lua-types-begin btech 00530
//|---Read one part's stored quantity; absent stock reports zero.
//|---@param target DbRef|Object
//|---@param part BattlePartRef
//|---@return integer quantity
//|function btech_parts.store_quantity(target, part) end
// lua-types-end

// lua-types-begin btech 00531
//|---List positive registered stock rows in native inventory order.
//|---@param target DbRef|Object
//|---@return BattlePartStack[] stores
//|function btech_parts.stores(target) end
// lua-types-end
