Color = require("color").Color

local function render_contents(ctx)
  local rendered = {}

  for _, object in ipairs(mux.contents(ctx.object)) do
    if mux.contents_visible(ctx.object, ctx.enactor, object) then
      local name = mux.object_name(object)
      if mux.object_type(object) == "player" then
        name = Color.ANSI_BRIGHT_WHITE .. name .. Color.ANSI_NORMAL
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

local function render_internal_appearance(ctx)
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
    render_contents=render_contents,
    render_exits=render_exits,
    render_internal_appearance=render_internal_appearance,
}
