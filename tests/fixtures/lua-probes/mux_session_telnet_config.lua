-- This probe exercises invalid calls and native internals on purpose.
---@diagnostic disable: missing-parameter, param-type-mismatch, redundant-parameter
-- Differential probe: mux.session, mux.telnet, mux.config and top-level mux.
-- Oracles: btmux-khi mux_session_bindings.c, mux_telnet_bindings.c,
-- mux_config_bindings.c and mux_package.c at the pinned revision.
local function json_string(value)
  return '"' .. value:gsub('[%z\1-\31\\"]', function(byte)
    local escapes = { ['\\'] = '\\\\', ['"'] = '\\"', ['\b'] = '\\b',
      ['\f'] = '\\f', ['\n'] = '\\n', ['\r'] = '\\r', ['\t'] = '\\t' }
    return escapes[byte] or string.format('\\u%04x', string.byte(byte))
  end) .. '"'
end

local function json(value)
  local kind = type(value)
  if kind == 'nil' then return 'null' end
  if kind == 'boolean' or kind == 'number' then return tostring(value) end
  if kind == 'string' then return json_string(value) end
  assert(kind == 'table', 'probe JSON only accepts plain records')
  local out = {}
  for index = 1, #value do out[#out + 1] = json(value[index]) end
  if #out > 0 then return '[' .. table.concat(out, ',') .. ']' end
  local keys = {}
  for key in pairs(value) do keys[#keys + 1] = key end
  table.sort(keys)
  for _, key in ipairs(keys) do
    out[#out + 1] = json_string(key) .. ':' .. json(value[key])
  end
  return '{' .. table.concat(out, ',') .. '}'
end

local function descriptor(value)
  local kind = type(value)
  local result = { type = kind }
  if kind == 'nil' then return result end
  if kind == 'number' then
    result.value = string.format('%.17g', value)
  elseif kind == 'boolean' then
    result.value = value
  elseif kind == 'string' then
    result.bytes = (value:gsub('.', function(byte) return string.format('%02x', string.byte(byte)) end))
  elseif kind == 'userdata' then
    result.text = tostring(value)
  elseif kind == 'table' then
    local ordered = {}
    for key, item in pairs(value) do
      ordered[#ordered + 1] = { sort = type(key) .. ':' .. tostring(key), key = key, value = item }
    end
    table.sort(ordered, function(left, right) return left.sort < right.sort end)
    result.entries = {}
    for _, entry in ipairs(ordered) do
      result.entries[#result.entries + 1] = {
        key = descriptor(entry.key), value = descriptor(entry.value),
      }
    end
  end
  return result
end

local function returns(...)
  local result = { count = select('#', ...), values = {} }
  for index = 1, result.count do
    result.values[index] = descriptor(select(index, ...))
  end
  return result
end

local function capture(call)
  local function pack(...) return { n = select('#', ...), ... } end
  local packed = pack(pcall(call))
  local failure = packed[2]
  if type(failure) == 'table' then
    failure = { code = failure.code, message = failure.message, detail = failure.detail }
  end
  return { count = packed.n, ok = descriptor(packed[1]), error = descriptor(failure) }
end

local function run(descriptor_id)
  local players = mux.session.connected_players()
  local summary = mux.session.who_summary()
  local projected = {}
  for index, player in ipairs(players) do
    projected[index] = {
      object = descriptor(tostring(player.object)),
      name = descriptor(player.name),
      connected_for = descriptor(type(player.connected_for)),
      idle_for = descriptor(type(player.idle_for)),
    }
  end
  return json({
    session = {
      connected_count = descriptor(#players),
      players = projected,
      connected_extra = capture(function() return #mux.session.connected_players('ignored') end),
      who_summary = returns(summary.hidden, summary.record, summary.maximum),
      who_summary_field_types = descriptor(type(summary.hidden)),
      flow_missing_descriptor = capture(function()
        return mux.session.flow_start(999999, 'lua_parity_probe.lua', 'step')
      end),
      flow_missing_module = capture(function()
        return mux.session.flow_start(descriptor_id, 'missing_flow_module.lua', 'step')
      end),
      flow_missing_step = capture(function()
        return mux.session.flow_start(descriptor_id, 'lua_parity_probe.lua', 'no_such_step')
      end),
      flow_bad_descriptor_type = capture(function()
        return mux.session.flow_start(false, 'lua_parity_probe.lua', 'step')
      end),
      flow_bad_module_type = capture(function()
        return mux.session.flow_start(descriptor_id, false, 'step')
      end),
      flow_omitted = capture(function() return mux.session.flow_start() end),
      flow_extra_ignored = capture(function()
        return mux.session.flow_start(descriptor_id, 'lua_parity_probe.lua', 'no_such_step', 'ignored')
      end),
    },
    telnet = {
      has_var_absent = capture(function()
        return mux.telnet.environment_has(descriptor_id, 'var', 'USER')
      end),
      has_uservar_absent = capture(function()
        return mux.telnet.environment_has(descriptor_id, 'uservar', 'USERVAR')
      end),
      get_var_absent = capture(function()
        return mux.telnet.environment_get(descriptor_id, 'var', 'USER')
      end),
      get_uservar_absent = capture(function()
        return mux.telnet.environment_get(descriptor_id, 'uservar', 'USERVAR')
      end),
      get_binary_name = capture(function()
        return mux.telnet.environment_get(descriptor_id, 'var', 'na\0me')
      end),
      no_descriptor = capture(function()
        return mux.telnet.environment_has(999999, 'var', 'USER')
      end),
      bad_kind = capture(function()
        return mux.telnet.environment_has(descriptor_id, 'group', 'USER')
      end),
      kind_nul_truncates = capture(function()
        return mux.telnet.environment_has(descriptor_id, 'var\0junk', 'USER')
      end),
      kind_number = capture(function()
        return mux.telnet.environment_has(descriptor_id, 7, 'USER')
      end),
      bad_descriptor_type = capture(function()
        return mux.telnet.environment_get(false, 'var', 'USER')
      end),
      bad_name_type = capture(function()
        return mux.telnet.environment_get(descriptor_id, 'var', false)
      end),
      omitted_args = capture(function() return mux.telnet.environment_has() end),
      extra_args = capture(function()
        return mux.telnet.environment_has(descriptor_id, 'var', 'USER', 'ignored')
      end),
    },
    config = {
      max_players = capture(function() return mux.config.get('max_players') end),
      fork_dump = capture(function() return mux.config.get('fork_dump') end),
      game_database = capture(function() return mux.config.get('game_database') end),
      unknown = capture(function() return mux.config.get('parity_absent_directive') end),
      section_name = capture(function() return mux.config.get('mux') end),
      number_name = capture(function() return mux.config.get(42) end),
      nul_name = capture(function() return mux.config.get('max_players\0ignored') end),
      omitted = capture(function() return mux.config.get() end),
      extra = capture(function() return type(mux.config.get('max_players', 'ignored')) end),
    },
    top = {
      check_db = capture(function() return mux.check_db() end),
      check_db_extra = capture(function() return mux.check_db('ignored', 'more') end),
      log_missing_file = capture(function() return mux.log('parity_absent.log', 'paired') end),
      log_empty_message = capture(function() return mux.log('parity_absent_too.log', '') end),
      log_dotdot = capture(function() return mux.log('../escape.log', 'paired') end),
      log_slash = capture(function() return mux.log('a/b.log', 'paired') end),
      log_long_name = capture(function()
        return mux.log(string.rep('x', 201) .. '.log', 'paired')
      end),
      log_nul_filename = capture(function() return mux.log('bad\0name', 'ok') end),
      log_nul_message = capture(function() return mux.log('parity.log', 'bad\0message') end),
      log_bad_filename_type = capture(function() return mux.log(false, 'ok') end),
      log_omitted = capture(function() return mux.log() end),
      log_extra = capture(function()
        return mux.log('parity_absent.log', 'paired', 'ignored')
      end),
    },
  })
end

local function emit(target, payload, descriptor_id)
  local size = 3000
  local count = math.max(1, math.ceil(#payload / size))
  for index = 1, count do
    local chunk = payload:sub(((index - 1) * size) + 1, index * size)
    mux.world.pemit(target, 'LUA_PARITY:' .. index .. '/' .. count .. ':' .. chunk)
  end
end

return {commands={{
  name='lua-parity-probe', permission='everyone', pattern='^luaparity$',
  handler=function(ctx)
    emit(ctx.enactor, run(ctx.descriptor), ctx.descriptor)
    return true
  end,
}}}
