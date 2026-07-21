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

-- ## Appearance definitions ## --

local ANSI_BRIGHT_WHITE = "\27[1;37m"
local ANSI_NORMAL = "\27[0m"

local function render_contents(ctx)
  local rendered = {}

  for _, object in ipairs(mux.contents(ctx.object)) do
    if mux.contents_visible(ctx.object, ctx.enactor, object) then
      local name = mux.object_name(object)
      if mux.object_type(object) == "player" then
        name = ANSI_BRIGHT_WHITE .. name .. ANSI_NORMAL
      end
      rendered[#rendered + 1] = name
    end
  end
  return rendered
end

local function render_exits(ctx)
  local rendered = {}

  for _, exit in ipairs(mux.exits(ctx.object)) do
    if mux.exits_visible(ctx.object, ctx.enactor, exit) then
      local stored_name = mux.object_name(exit)
      local name, aliases = stored_name:match("^([^;]+);?(.*)$")
      local first_alias = aliases:match("^([^;]+)")
      if first_alias then
        rendered[#rendered + 1] = "(" .. first_alias .. ") " .. name
      else
        rendered[#rendered + 1] = name
      end
    end
  end
  return rendered
end

local function render_appearance(ctx)
  local lines = {
    mux.object_name(ctx.object),
    mux.object_description(ctx.object) or "",
  }
  local contents = {}
  local exits = {}

  if mux.object_type(ctx.object) ~= "exit" then
    contents = render_contents(ctx)
    exits = render_exits(ctx)
  end

  if #contents > 0 then
    lines[#lines + 1] = "Contents:"
    for _, content in ipairs(contents) do
      lines[#lines + 1] = content
    end
  end
  if #exits > 0 then
    lines[#lines + 1] = "Obvious exits:"
    for _, exit in ipairs(exits) do
      lines[#lines + 1] = exit
    end
  end
  return table.concat(lines, "\r\n")
end

return {
  internal_appearance = render_appearance,
  external_appearance = render_appearance,
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
        mux.notify(ctx.object, "The Lua schedule has fired.")
      end,
    },
  },
}
