-- Typed persistent object state.
-- Attach with: @lua/parent <object>=counter.lua
return {
  commands = {
    {
      pattern = "^count$",
      handler = function(ctx)
        local state = mux.object(ctx.object):state("counter")
        local count = state:get("count", 0) + 1

        state:set("count", count)
        mux.notify(ctx.enactor, "This object has been used " .. count .. " times.")
        return true
      end,
    },
  },
  events = {},
}
