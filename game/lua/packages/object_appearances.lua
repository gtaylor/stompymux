local ROOM_COLUMN_WIDTH = 38
local ROOM_HEADER_COLUMN_WIDTH = ROOM_COLUMN_WIDTH
local ROOM_PLAYER_NAME_WIDTH = 23
local ROOM_EXIT_NAME_WIDTH = 32

local function truncate(value, width)
  return mux.truncate_text(value, width)
end

local function pad_right(value, width)
  return value .. string.rep(" ", math.max(0, width - mux.text_width(value)))
end

local function quote_link_target(value)
  return value:gsub("\\", "\\\\"):gsub('"', '\\"')
end

local function send_link(value, command)
  return mux.markup('[send="' .. quote_link_target(command) .. '"]' .. value .. "[/]")
end

local function render_content_name(object)
  local name = object.name

  if object.type == "player" then
    name = mux.style(name, { foreground = "bright-white" })
  end
  return name
end

local function render_contents(ctx)
  local rendered = {}
  local container = mux.object(ctx.object)

  for _, object in ipairs(container:contents()) do
    if container:contents_visible(ctx.enactor, object) then
      rendered[#rendered + 1] = render_content_name(object)
    end
  end
  return rendered
end

local function render_exits(ctx)
  local rendered = {}
  local location = mux.object(ctx.object)

  for _, exit in ipairs(location:exits()) do
    if location:exits_visible(ctx.enactor, exit) then
      local stored_name = exit.name
      local name, aliases = stored_name:match("^([^;]+);?(.*)$")
      local first_alias = aliases:match("^([^;]+)")
      local command = mux.strip_style(first_alias or name)
      if first_alias then
        rendered[#rendered + 1] = send_link("(" .. first_alias .. ") " .. name, command)
      else
        rendered[#rendered + 1] = send_link(name, command)
      end
    end
  end
  return rendered
end

local function render_room_contents_and_players(ctx)
  local contents = {}
  local players = {}
  local container = mux.object(ctx.object)

  for _, object in ipairs(container:contents()) do
    if container:contents_visible(ctx.enactor, object) then
      if object.type == "player" then
        if object.dbref ~= ctx.enactor then
          players[#players + 1] = object.name
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
  local location = mux.object(ctx.object)

  for _, exit in ipairs(location:exits()) do
    if location:exits_visible(ctx.enactor, exit) then
      local stored_name = exit.name
      local name, aliases = stored_name:match("^([^;]+);?(.*)$")
      local first_alias = aliases:match("^([^;]+)")
      local passes_enter_lock = exit:enter_lock_passes(ctx.enactor)

      rendered[#rendered + 1] = {
        alias = first_alias and "{" .. first_alias .. "}" or "",
        alias_color = passes_enter_lock and "green" or "red",
        name = name,
        name_color = passes_enter_lock and "bright-green" or "bright-red",
        command = mux.strip_style(first_alias or name),
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
    rendered_player_header = mux.style(player_header, { foreground = "bright-yellow" })
  end
  if exit_header ~= "" then
    rendered_exit_header = mux.style(exit_header, { foreground = "bright-yellow" })
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
      player_column = " " .. mux.style(name, { foreground = "bright-white" })
      player_column = player_column .. string.rep(" ", ROOM_COLUMN_WIDTH - mux.text_width(name) - 1)
    end
    if exit then
      local alias = ""
      local name = truncate(exit.name, ROOM_EXIT_NAME_WIDTH)

      if exit.alias ~= "" then
        alias = mux.style("{", { foreground = "white" })
          .. mux.style(exit.alias:sub(2, -2), { foreground = exit.alias_color })
          .. mux.style("}", { foreground = "white" })
        alias = send_link(alias, exit.command)
      end
      exit_column = alias
        .. string.rep(" ", math.max(0, 4 - #exit.alias))
        .. " "
        .. send_link(mux.style(name, { foreground = exit.name_color }), exit.command)
    end
    lines[#lines + 1] = pad_right(player_column, ROOM_COLUMN_WIDTH) .. exit_column
  end
  return lines
end

local function render_internal_appearance(ctx)
  local object = mux.object(ctx.object)
  local lines = {
    object.name,
    object.description or "",
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
    lines[#lines + 1] = " " .. mux.style("Contents:", { foreground = "bright-yellow" })
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
