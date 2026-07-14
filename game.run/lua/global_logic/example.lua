-- Globally available after @luareload.
-- This command intentionally uses a distinct name to avoid object-local hello
-- examples: type "global-hello" from anywhere in the game.
return {
  commands = {
    {
      pattern = "^global%-hello$",
      handler = function(ctx)
        mux.notify(ctx.enactor, "Hello, world, from a global Lua command!")
        return true
      end,
    },
  },
  schedules = {
    {
      name = "hourly_maintenance",
      cron = "0 * * * *",
      handler = function(ctx)
        -- Global scheduled events have ctx.scope == "global" and no enactor.
      end,
    },
  },
}
