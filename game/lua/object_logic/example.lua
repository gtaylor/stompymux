-- Attach with: @lua/parent <object>=example.lua
-- Reload changed modules with: @lua/reload

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
    on_enter = at_enter,
  },
  locks = {
    use = function(ctx)
      if ctx.subject == ctx.enactor then
        return true
      end
      return {
        passes = false,
        enactor_message = "You cannot use that.",
        other_message = "tries to use it, but cannot.",
      }
    end,
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
