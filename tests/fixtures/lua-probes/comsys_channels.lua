-- This probe exercises invalid calls and native internals on purpose.
---@diagnostic disable: assign-type-mismatch, inject-field, missing-parameter, param-type-mismatch, redundant-parameter
-- Differential probe: mux.comsys channel registry, handles, flags and errors.
-- Oracle: btmux-khi/src/mux/lua/packages/mux/comsys/mux_comsys_bindings.c and
-- mux_comsys_channel_flag_bindings.c at the pinned revision.
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

-- Record errors as plain records through the descriptor path so the harness
-- normalizer can rewrite incidental source spellings inside message bytes.
local function capture(call)
  local function pack(...) return { n = select('#', ...), ... } end
  local packed = pack(pcall(call))
  local failure = packed[2]
  if type(failure) == 'table' then
    failure = { code = failure.code, message = failure.message, detail = failure.detail }
  end
  return { count = packed.n, ok = descriptor(packed[1]), error = descriptor(failure) }
end

local function names_of(channels)
  local names = {}
  for index, channel in ipairs(channels) do names[index] = channel:name() end
  return names
end

local function run()
  local god = mux.world.object(1)
  local initial = names_of(mux.comsys.list_channels())
  local created = mux.comsys.create_channel('parity')
  local looked_up = mux.comsys.channel('PARITY')
  local list_after_create = names_of(mux.comsys.list_channels())
  local flags = created:flags()
  local flags_tostring = tostring(flags)
  local flag_set = flags:list()
  local had_public = flags:has(mux.comsys.flags.PUBLIC)
  local add_public = flags:add(mux.comsys.flags.PUBLIC)
  local add_public_again = flags:add(mux.comsys.flags.PUBLIC)
  local list_public = flags:list()
  local had_loud = flags:has(mux.comsys.flags.LOUD)
  local add_loud = flags:add(mux.comsys.flags.LOUD)
  local remove_public = flags:remove(mux.comsys.flags.PUBLIC)
  local remove_public_again = flags:remove(mux.comsys.flags.PUBLIC)
  local list_cleared = flags:list()
  local transparent_had = flags:has(mux.comsys.flags.TRANSPARENT)
  local add_transparent = flags:add(mux.comsys.flags.TRANSPARENT)
  local remove_loud = flags:remove(mux.comsys.flags.LOUD)
  local remove_transparent = flags:remove(mux.comsys.flags.TRANSPARENT)
  local getters = returns(
    created:name(), created:object(), created:max_user_count(), created:user_count(), created:message_count()
  )
  local set_object_nil = returns(created:set_object(nil))
  local set_object_god = returns(created:set_object(god))
  local object_after_set = returns(created:object())
  local detached = returns(created:set_object(nil))
  local add_player = returns(created:add_player(god, 'par', false))
  local user_count_after_add = returns(created:user_count())
  local max_users_after_add = returns(created:max_user_count())
  local who_default = descriptor(created:who())
  local who_all = descriptor(created:who({ all = true }))
  local second = mux.comsys.create_channel('zulu')
  local second_add = capture(function() return second:add_player(god, 'par', true) end)
  local alias_in_use = capture(function() return created:add_player(god, 'par', false) end)
  mux.world.pemit(god, 'LUA_PARITY_OUTPUT_BEGIN')
  local emit = returns(created:emit('paired hello', {}))
  local emit_no_header = returns(created:emit('paired bare', { no_header = true }))
  local message_count_after_emit = returns(created:message_count())
  local boot_player = returns(created:boot_player(god))
  local user_count_after_boot = returns(created:user_count())
  local alias_after_boot = capture(function() return created:add_player(god, 'par', false) end)
  local destroyed = returns(mux.comsys.destroy_channel(created))
  local gone = capture(function() return mux.comsys.channel('parity') end)
  local stale_name = capture(function() return created:name() end)
  local stale_flags = capture(function() return flags:list() end)
  local recreated = capture(function() return mux.comsys.create_channel('parity') end)
  local stale_after_recreate = capture(function() return created:name() end)
  local destroy_second = returns(mux.comsys.destroy_channel(second))
  return json({
    __capture_output = true,
    initial_channels = initial,
    create_tostring = descriptor(tostring(created)),
    lookup_equal = descriptor(created == looked_up),
    lookup_tostring = descriptor(tostring(looked_up)),
    list_after_create = list_after_create,
    flags = {
      tostring = descriptor(flags_tostring),
      initial_list = descriptor(flag_set),
      had_public = descriptor(had_public), add_public = descriptor(add_public),
      add_public_again = descriptor(add_public_again), list_public = descriptor(list_public),
      had_loud = descriptor(had_loud), add_loud = descriptor(add_loud),
      remove_public = descriptor(remove_public),
      remove_public_again = descriptor(remove_public_again),
      list_cleared = descriptor(list_cleared),
      transparent_had = descriptor(transparent_had),
      add_transparent = descriptor(add_transparent),
      remove_loud = descriptor(remove_loud),
      remove_transparent = descriptor(remove_transparent),
      equality = descriptor(mux.comsys.flags.PUBLIC == mux.comsys.flags.PUBLIC),
      cross = descriptor(mux.comsys.flags.PUBLIC ~= mux.comsys.flags.LOUD),
      wrong_flag = capture(function() return flags:has(mux.world.flags.DARK) end),
      wrong_flag_false = capture(function() return flags:add(false) end),
    },
    getters = getters,
    set_object_nil = set_object_nil,
    set_object_god = set_object_god,
    object_after_set = object_after_set,
    detached = detached,
    membership = {
      add_player = add_player,
      user_count_after_add = user_count_after_add,
      max_users_after_add = max_users_after_add,
      who_default = who_default,
      who_all = who_all,
      second_add = second_add,
      alias_in_use = alias_in_use,
      boot_player = boot_player,
      user_count_after_boot = user_count_after_boot,
      alias_after_boot = alias_after_boot,
    },
    emit = emit,
    emit_no_header = emit_no_header,
    message_count_after_emit = message_count_after_emit,
    destroy = {
      destroyed = destroyed,
      gone = gone,
      stale_name = stale_name,
      stale_flags = stale_flags,
      recreated = recreated,
      stale_after_recreate = stale_after_recreate,
      destroy_second = destroy_second,
      stale_tostring = descriptor(tostring(created)),
      stale_equality = returns(created == created),
    },
    errors = (function()
      -- Dedicated live channel: the main flow already destroyed parity/zulu.
      local e = mux.comsys.create_channel('errch')
      local room = mux.world.object(0)
      return {
        channel_omitted = capture(function() return mux.comsys.channel() end),
        channel_false = capture(function() return mux.comsys.channel(false) end),
        channel_number_name = capture(function() return mux.comsys.channel(42) end),
        channel_nul = capture(function() return mux.comsys.channel('errch\0suffix') end),
        channel_missing = capture(function() return mux.comsys.channel('absent') end),
        create_omitted = capture(function() return mux.comsys.create_channel() end),
        create_empty = capture(function() return mux.comsys.create_channel('') end),
        create_space = capture(function() return mux.comsys.create_channel('a b') end),
        create_punctuation = capture(function() return mux.comsys.create_channel('a\xffb') end),
        create_nul = capture(function() return mux.comsys.create_channel('a\0b') end),
        create_long = capture(function() return mux.comsys.create_channel(string.rep('x', 50)) end),
        create_exists = capture(function() return mux.comsys.create_channel('ERRCH') end),
        destroy_false = capture(function() return mux.comsys.destroy_channel(false) end),
        destroy_nil = capture(function() return mux.comsys.destroy_channel(nil) end),
        destroy_number = capture(function() local f = mux.comsys.destroy_channel return f(5) end),
        list_extra = capture(function() return #mux.comsys.list_channels('ignored') end),
        emit_omitted_message = capture(function() return e:emit() end),
        emit_nul = capture(function() return e:emit('bad\0message') end),
        emit_invalid_utf8 = capture(function() return e:emit('bad\255message') end),
        emit_unknown_option = capture(function() return e:emit('x', { unknown = true }) end),
        emit_option_type = capture(function() return e:emit('x', { no_header = 1 }) end),
        emit_options_not_table = capture(function() return e:emit('x', false) end),
        who_unknown_option = capture(function() return e:who({ everything = 1 }) end),
        who_option_type = capture(function() return e:who({ all = 1 }) end),
        who_options_not_table = capture(function() return e:who(false) end),
        add_player_omitted = capture(function() return e:add_player() end),
        add_player_room = capture(function() return e:add_player(room, 'pp', false) end),
        add_player_missing_object = capture(function() return e:add_player(9999, 'pp', false) end),
        add_player_alias_type = capture(function() return e:add_player(god, 12, false) end),
        add_player_alias_empty = capture(function() return e:add_player(god, '', false) end),
        add_player_alias_long = capture(function() return e:add_player(god, 'toolong', false) end),
        add_player_alias_space = capture(function() return e:add_player(god, 'a b', false) end),
        add_player_quiet_type = capture(function() return e:add_player(god, 'pq', nil) end),
        boot_not_member = capture(function() return e:boot_player(room) end),
        boot_missing = capture(function() return e:boot_player() end),
        set_object_omitted = capture(function() return e:set_object() end),
        set_object_bad = capture(function() return e:set_object(false) end),
        set_object_missing = capture(function() return e:set_object(9999) end),
        handle_immutable = capture(function() e.new_field = true end),
        flags_wrong_type = capture(function() return e:flags():has(mux.world.flags.DARK) end),
        flags_wrong_false = capture(function() return e:flags():add(false) end),
        flags_tostring = descriptor(tostring(e:flags())),
        errors_destroyed = returns(mux.comsys.destroy_channel(e)),
      }
    end)(),
  })
end

local function emit(target, payload)
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
    emit(ctx.enactor, run())
    return true
  end,
}}}
