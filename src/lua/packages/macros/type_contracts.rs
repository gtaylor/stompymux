//! LuaLS declarations for trusted macro management.

// lua-types-begin mux 00150
//|---@class MacroSet
//|local MacroSet = {}
//|---@class MacroFlag
//|---@class MacroFlags
//|local MacroFlags = {}
//|---@class MacroFlagConstants
//|---@field LOCKED MacroFlag Blocks player definition edits and renaming, including by the owner.
//|---@field READ MacroFlag Allows other players to discover, inspect, and attach the set.
//|---@field WRITE MacroFlag Allows other players to edit the unlocked set.
//|---@class MacroEntry
//|---@field alias string
//|---@field expansion string
//|---@class MacroAttachment
//|---@field slot integer Zero-based slot, 0 through 4.
//|---@field set MacroSet
//|---@field selected boolean
//|---@class MuxMacroPackage
//|---@field flags MacroFlagConstants
//|local mux_macro = {}
// lua-types-end

// lua-types-begin mux 00151
//|---List all sets in current number order.
//|---Requires an active callback transaction; bypasses player permission checks.
//|---@return MacroSet[] sets
//|function mux_macro.list_sets() end
// lua-types-end

// lua-types-begin mux 00152
//|---Resolve a transient set number; absent sets return nil.
//|---Requires an active callback transaction; bypasses player permission checks.
//|---@param number integer Zero-based current set number.
//|---@return MacroSet? set
//|function mux_macro.set(number) end
// lua-types-end

// lua-types-begin mux 00153
//|---Create a private unlocked set without attaching it.
//|---Requires an active callback transaction; bypasses player permission checks.
//|---@param owner DbRef|Object
//|---@param description string
//|---@return MacroSet set
//|function mux_macro.create_set(owner, description) end
// lua-types-end

// lua-types-begin mux 00154
//|---Destroy a set and clear every attachment to it.
//|---Requires an active callback transaction; bypasses player permission checks.
//|---@param set MacroSet
//|function mux_macro.destroy_set(set) end
// lua-types-end

// lua-types-begin mux 00155
//|---Attach to the first free slot without changing editing selection.
//|---Requires an active callback transaction; bypasses player permission checks.
//|---@param player DbRef|Object
//|---@param set MacroSet
//|---@return integer slot
//|function mux_macro.attach(player, set) end
// lua-types-end

// lua-types-begin mux 00156
//|---List occupied slots in slot order.
//|---Requires an active callback transaction; bypasses player permission checks.
//|---@param player DbRef|Object
//|---@return MacroAttachment[] attachments
//|function mux_macro.list_player_sets(player) end
// lua-types-end

// lua-types-begin mux 00157
//|---Clear a slot and its editing selection, if selected.
//|---Requires an active callback transaction; bypasses player permission checks.
//|---@param player DbRef|Object
//|---@param slot integer Zero-based slot, 0 through 4.
//|---@return boolean removed
//|function mux_macro.detach(player, slot) end
// lua-types-end

// lua-types-begin mux 00158
//|---Current zero-based set number; changes after earlier sets are destroyed.
//|---Requires an active callback transaction and a live handle.
//|---@return integer number
//|function MacroSet:number() end
// lua-types-end

// lua-types-begin mux 00159
//|---Read the description.
//|---Requires an active callback transaction and a live handle.
//|---@return string description
//|function MacroSet:description() end
// lua-types-end

// lua-types-begin mux 00160
//|---Read the owner.
//|---Requires an active callback transaction and a live handle.
//|---@return Object owner
//|function MacroSet:owner() end
// lua-types-end

// lua-types-begin mux 00161
//|---Set any live object as owner.
//|---Requires an active callback transaction and a live handle.
//|---@param owner DbRef|Object
//|function MacroSet:set_owner(owner) end
// lua-types-end

// lua-types-begin mux 00162
//|---List detached records in case-insensitive alias order.
//|---Requires an active callback transaction and a live handle.
//|---@return MacroEntry[] entries
//|function MacroSet:list_macros() end
// lua-types-end

// lua-types-begin mux 00163
//|---Add a new alias; duplicates raise mux.macro.exists.
//|---Requires an active callback transaction and a live handle.
//|---@param alias string
//|---@param expansion string
//|function MacroSet:add_macro(alias, expansion) end
// lua-types-end

// lua-types-begin mux 00164
//|---Update expansion while retaining alias spelling; missing aliases raise mux.macro.not_found.
//|---Requires an active callback transaction and a live handle.
//|---@param alias string
//|---@param expansion string
//|function MacroSet:update_macro(alias, expansion) end
// lua-types-end

// lua-types-begin mux 00165
//|---Delete by case-insensitive alias; missing aliases raise mux.macro.not_found.
//|---Requires an active callback transaction and a live handle.
//|---@param alias string
//|function MacroSet:delete_macro(alias) end
// lua-types-end

// lua-types-begin mux 00166
//|---Return the live permission collection.
//|---Requires an active callback transaction and a live handle.
//|---@return MacroFlags flags
//|function MacroSet:flags() end
// lua-types-end

// lua-types-begin mux 00167
//|---Access the owning set permissions; requires a live handle and callback transaction.
//|---@return MacroFlag[] flags
//|function MacroFlags:list() end
// lua-types-end

// lua-types-begin mux 00168
//|---Access the owning set permissions; requires a live handle and callback transaction.
//|---@param flag MacroFlag
//|---@return boolean enabled
//|function MacroFlags:has(flag) end
// lua-types-end

// lua-types-begin mux 00169
//|---Access the owning set permissions; requires a live handle and callback transaction.
//|---@param flag MacroFlag
//|function MacroFlags:add(flag) end
// lua-types-end

// lua-types-begin mux 00170
//|---Access the owning set permissions; requires a live handle and callback transaction.
//|---@param flag MacroFlag
//|function MacroFlags:remove(flag) end
// lua-types-end

// lua-types-begin mux 00171
//|mux.macro = mux_macro
// lua-types-end
