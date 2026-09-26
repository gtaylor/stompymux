//! LuaLS contract blocks for the btech database surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00418
//|---Request persistence of the current world at transaction commit, even if unchanged.
//|---@param actor integer Wizard requesting the checkpoint.
//|---@return boolean queued Success is published only after persistence; rollback cancels the request.
//|function btech_database.save(actor) end
// lua-types-end
