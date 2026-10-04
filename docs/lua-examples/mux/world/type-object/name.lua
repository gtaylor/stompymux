-- In a command callback, ctx.object identifies the object running the command.
local function show_name(ctx)
  local object = mux.world.object(ctx.object)
  local name = object:name()
  mux.world.pemit(ctx.enactor, "This object is named " .. name)
end
