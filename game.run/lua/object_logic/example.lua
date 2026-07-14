-- Attach with: @luaparent <object>=example.lua
-- Reload changed modules with: @luareload

-- ## Command definitions ## --

local function hello_command(ctx, name)
  mux.notify(ctx.enactor, "Hello " .. (name ~= "" and name or "there") .. "!")
  return true
end

-- ## Event definitions ## --

local function at_enter(ctx)
  mux.notify(ctx.enactor, "You trigger the Lua enter event.")
end

return {
  commands = {
    {
      pattern = "^hello%s*(.*)$",
      handler = hello_command,
    },
  },
  events = {
    aenter = at_enter,
  },
  schedules = {
    {
      name = "hourly_notice",
      cron = "0 * * * *",
      handler = function(ctx)
        mux.notify(ctx.object, "The Lua schedule has fired.")
      end,
    },
  },
}
