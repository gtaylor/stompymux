-- Minimal command example.
-- Attach with: @luaparent <object>=hello.lua
return {
  commands = {
    {
      pattern = "^hello$",
      handler = function(ctx)
        mux.notify(ctx.enactor, "Hello, world!")
        return true
      end,
    },
  },
  events = {},
}
