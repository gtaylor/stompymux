local Color = require("color")

local ROOM_COLUMN_WIDTH = 38
local ROOM_HEADER_COLUMN_WIDTH = ROOM_COLUMN_WIDTH
local ROOM_PLAYER_NAME_WIDTH = 23
local ROOM_EXIT_NAME_WIDTH = 32

local function truncate(value, width)
  if #value > width then
    return value:sub(1, width)
  end
  return value
end

local function pad_right(value, width)
  return value .. string.rep(" ", math.max(0, width - #value))
end

local function render_content_name(object)
  local name = mux.object_name(object)

  if mux.object_type(object) == "player" then
    name = Color.ANSI_BRIGHT_WHITE .. name .. Color.ANSI_NORMAL
  end
  return name
end

local function render_contents(ctx)
  local rendered = {}

  for _, object in ipairs(mux.contents(ctx.object)) do
    if mux.contents_visible(ctx.object, ctx.enactor, object) then
      rendered[#rendered + 1] = render_content_name(object)
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

local function render_room_contents_and_players(ctx)
  local contents = {}
  local players = {}

  for _, object in ipairs(mux.contents(ctx.object)) do
    if mux.contents_visible(ctx.object, ctx.enactor, object) then
      if mux.object_type(object) == "player" then
        if object ~= ctx.enactor then
          players[#players + 1] = mux.object_name(object)
        end
      else
        contents[#contents + 1] = render_content_name(object)
      end
    end
  end
  return contents, players
end

local function render_room_exits(ctx)
  local rendered = {}

  for _, exit in ipairs(mux.exits(ctx.object)) do
    if mux.exits_visible(ctx.object, ctx.enactor, exit) then
      local stored_name = mux.object_name(exit)
      local name, aliases = stored_name:match("^([^;]+);?(.*)$")
      local first_alias = aliases:match("^([^;]+)")
      local passes_enter_lock = mux.exit_enter_lock_passes(exit, ctx.enactor)

      rendered[#rendered + 1] = {
        alias = first_alias and "{" .. first_alias .. "}" or "",
        alias_color = passes_enter_lock and Color.ANSI_DARK_GREEN or Color.ANSI_DARK_RED,
        name = name,
        name_color = passes_enter_lock and Color.ANSI_BRIGHT_GREEN or Color.ANSI_BRIGHT_RED,
      }
    end
  end
  return rendered
end

local function render_room_columns(players, exits)
  local lines = {}
  local player_header = #players > 0 and " Players:" or ""
  local exit_header = #exits > 0 and "Obvious Exits:" or ""
  local rendered_player_header = player_header
  local rendered_exit_header = exit_header

  if player_header ~= "" then
    rendered_player_header = Color.ANSI_BRIGHT_YELLOW .. player_header .. Color.ANSI_NORMAL
  end
  if exit_header ~= "" then
    rendered_exit_header = Color.ANSI_BRIGHT_YELLOW .. exit_header .. Color.ANSI_NORMAL
  end
  lines[#lines + 1] = rendered_player_header
    .. string.rep(" ", ROOM_HEADER_COLUMN_WIDTH - #player_header)
    .. rendered_exit_header
  for index = 1, math.max(#players, #exits) do
    local player = players[index]
    local exit = exits[index]
    local player_column = ""
    local exit_column = ""

    if player then
      local name = truncate(player, ROOM_PLAYER_NAME_WIDTH)
      player_column = " " .. Color.ANSI_BRIGHT_WHITE .. name .. Color.ANSI_NORMAL
      player_column = player_column .. string.rep(" ", ROOM_COLUMN_WIDTH - #name - 1)
    end
    if exit then
      local alias = ""
      local name = truncate(exit.name, ROOM_EXIT_NAME_WIDTH)

      if exit.alias ~= "" then
        alias = Color.ANSI_WHITE
          .. "{"
          .. Color.ANSI_NORMAL
          .. exit.alias_color
          .. exit.alias:sub(2, -2)
          .. Color.ANSI_NORMAL
          .. Color.ANSI_WHITE
          .. "}"
          .. Color.ANSI_NORMAL
      end
      exit_column = alias
        .. string.rep(" ", math.max(0, 4 - #exit.alias))
        .. " "
        .. exit.name_color
        .. name
        .. Color.ANSI_NORMAL
    end
    lines[#lines + 1] = pad_right(player_column, ROOM_COLUMN_WIDTH) .. exit_column
  end
  return lines
end

local function render_internal_appearance(ctx)
  local lines = {
    mux.object_name(ctx.object),
    mux.object_description(ctx.object) or "",
  }
  local contents, players = render_room_contents_and_players(ctx)
  local exits = render_room_exits(ctx)

  if #contents > 0 or #players > 0 or #exits > 0 then
    lines[#lines + 1] = ""
  end
  if #players > 0 or #exits > 0 then
    for _, line in ipairs(render_room_columns(players, exits)) do
      lines[#lines + 1] = line
    end
  end
  if #contents > 0 then
    if #players > 0 or #exits > 0 then
      lines[#lines + 1] = ""
    end
    lines[#lines + 1] = " " .. Color.ANSI_BRIGHT_YELLOW .. "Contents:" .. Color.ANSI_NORMAL
    for _, content in ipairs(contents) do
      lines[#lines + 1] = " " .. content
    end
  end
  return table.concat(lines, "\r\n")
end

return {
  render_contents = render_contents,
  render_exits = render_exits,
  render_internal_appearance = render_internal_appearance,
}
