-- Attach with: @lua/parent <object>=events/enter_notice.lua
return {
  commands = {},
  events = {
    on_enter = function(ctx)
      mux.world.pemit(ctx.enactor, "You trigger the Lua enter event.")
    end,
  },
}
