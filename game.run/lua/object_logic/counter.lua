-- Persistent object state using normal MUX attributes.
-- Attach with: @luaparent <object>=counter.lua
return {
  commands = {
    {
      pattern = "^count$",
      handler = function(ctx)
        local count = tonumber(mux.attr_get(ctx.object, "LuaCount") or "0") + 1

        mux.attr_set(ctx.object, "LuaCount", tostring(count))
        mux.notify(ctx.enactor, "This object has been used " .. count .. " times.")
        return true
      end,
    },
  },
  events = {},
}
