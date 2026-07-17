-- The player-facing WHO command. Wizards use @who for connection details.

local function format_connected(seconds)
  local days = math.floor(seconds / 86400)
  local hours = math.floor(seconds % 86400 / 3600)
  local minutes = math.floor(seconds % 3600 / 60)

  if days > 0 then
    return string.format("%dd %02d:%02d", days, hours, minutes)
  end
  return string.format("%02d:%02d", hours, minutes)
end

local function format_idle(seconds)
  if seconds > 600 then
    return "0s"
  end
  local days = math.floor(seconds / 86400)
  local hours = math.floor(seconds % 86400 / 3600)
  local minutes = math.floor(seconds % 3600 / 60)
  seconds = seconds % 60

  if days > 0 then
    return string.format("%dd", days)
  elseif hours > 0 then
    return string.format("%dh", hours)
  elseif minutes > 0 then
    return string.format("%dm", minutes)
  end
  return string.format("%ds", seconds)
end

local function has_prefix(name, prefix)
  return prefix == "" or name:lower():sub(1, #prefix) == prefix
end

return {
  commands = {
    {
      pattern = "^[Ww][Hh][Oo](.*)$",
      handler = function(ctx, query)
        local prefix = query:match("^%s*(.-)%s*$"):lower()
        local players = mux.connected_players()
        local count = 0

        mux.notify(ctx.enactor, "Player Name         On For  Idle ")
        for _, player in ipairs(players) do
          if has_prefix(player.name, prefix) then
            count = count + 1
            mux.notify(ctx.enactor, string.format("%-16s%10s %5s",
              player.name:sub(1, 16), format_connected(player.connected_for),
              format_idle(player.idle_for)))
          end
        end

        local summary = mux.who_summary()
        local maximum = summary.maximum and tostring(summary.maximum) or "no"
        if summary.hidden > 0 then
          mux.notify(ctx.enactor, string.format(
            "%d Visible Player%slogged in, (%d %s hidden), %d record, %s maximum.",
            count, count == 1 and " " or "s ", summary.hidden,
            summary.hidden == 1 and "is" or "are", summary.record, maximum))
        else
          mux.notify(ctx.enactor, string.format(
            "%d Player%slogged in, %d record, %s maximum.", count,
            count == 1 and " " or "s ", summary.record, maximum))
        end
        return true
      end,
    },
  },
}
