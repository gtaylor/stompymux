-- Attach with: @lua/parent <object>=example.lua
-- Reload changed modules with: @lua/reload
ObjectAppearances = require("object_appearances")

-- ## Command definitions ## --

local function hello_command(ctx, name)
  mux.world.pemit(ctx.enactor, "Hello " .. (name ~= "" and name or "there") .. "!")
  return true
end

-- ## Event definitions ## --

local function at_enter(ctx)
  mux.world.pemit(ctx.enactor, "You trigger the Lua enter event.")
end

return {
  internal_appearance = ObjectAppearances.render_internal_appearance,
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
  messages = {
    use = function(ctx)
      return {
        enactor_message = "You activate the example.",
        other_message = "activates the example.",
      }
    end,
  },
  schedules = {
    {
      name = "hourly_notice",
      cron = "0 * * * *",
      handler = function(ctx)
        mux.world.pemit(ctx.object, "The Lua schedule has fired.")
      end,
    },
  },
}
