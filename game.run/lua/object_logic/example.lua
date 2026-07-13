-- Attach with: @luaparent <object>=example.lua
-- Reload changed modules with: @luareload
return {
  commands = {
    {
      pattern = "^hello%s*(.*)$",
      handler = function(ctx, name)
        mux.notify(ctx.enactor, "Hello " .. (name ~= "" and name or "there") .. "!")
        return true
      end,
    },
  },
  events = {
    aenter = function(ctx)
      mux.notify(ctx.enactor, "You trigger the Lua enter event.")
    end,
  },
}
