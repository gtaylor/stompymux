-- Attach with: @luaparent <object>=events/enter_notice.lua
return {
  commands = {},
  events = {
    aenter = function(ctx)
      mux.notify(ctx.enactor, "You trigger the Lua enter event.")
    end,
  },
}
