-- Attach with: @lua/parent <object>=events/enter_notice.lua
return {
  commands = {},
  events = {
    on_enter = function(ctx)
      mux.notify(ctx.enactor, "You trigger the Lua enter event.")
    end,
  },
}
