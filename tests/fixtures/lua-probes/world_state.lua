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

local function descriptor(value, seen)
  local kind = type(value)
  local result = { type = kind }
  if kind == 'nil' then return result end
  if kind == 'number' then
    if value ~= value then result.value = 'nan'
    elseif value == math.huge then result.value = 'positive_infinity'
    elseif value == -math.huge then result.value = 'negative_infinity'
    else result.value = string.format('%.17g', value) end
  elseif kind == 'boolean' then
    result.value = value
  elseif kind == 'string' then
    result.bytes = (value:gsub('.', function(byte) return string.format('%02x', string.byte(byte)) end))
  elseif kind == 'userdata' then
    result.text = tostring(value)
  elseif kind == 'table' then
    seen = seen or {}
    if seen[value] then result.cycle = true; return result end
    seen[value] = true
    local ordered = {}
    for key, item in pairs(value) do
      ordered[#ordered + 1] = { sort = type(key) .. ':' .. tostring(key), key = key, value = item }
    end
    table.sort(ordered, function(left, right) return left.sort < right.sort end)
    result.entries = {}
    for _, entry in ipairs(ordered) do
      result.entries[#result.entries + 1] = {
        key = descriptor(entry.key, seen), value = descriptor(entry.value, seen),
      }
    end
    seen[value] = nil
  end
  return result
end

local function returns(...)
  local result = { count = select('#', ...), values = {} }
  for index = 1, result.count do
    local value = select(index, ...)
    result.values[index] = descriptor(value)
  end
  return result
end

local function namespace(name, value)
  local row = { name = name, type = type(value), keys = {} }
  if type(value) == 'table' then
    for key in pairs(value) do row.keys[#row.keys + 1] = type(key) .. ':' .. tostring(key) end
    table.sort(row.keys)
  elseif type(value) == 'userdata' then
    row.text = tostring(value)
  end
  return row
end


local function capture(call)
  local function pack(...) return {n=select('#', ...), ...} end
  local packed = pack(pcall(call))
  return {count=packed.n, ok=descriptor(packed[1]), error=descriptor(packed[2])}
end

local function capture_error_pcall(call)
  local function pack(...) return {n=select('#', ...), ...} end
  local packed = pack(mux.error.pcall(call))
  -- Tracebacks embed host-specific source spellings; compare code and message only.
  local failure = packed[2]
  if type(failure) == 'table' then
    failure = {code=failure.code, message=failure.message}
  end
  return {count=packed.n, ok=descriptor(packed[1]), error=descriptor(failure)}
end

local function ids(rows)
  local result = {}
  for _, object in ipairs(rows) do result[#result + 1] = object:dbref() end
  return result
end

local function catalogue(namespace, names)
  local values = {}
  for _, name in ipairs(names) do
    local value = namespace[name]
    values[#values + 1] = {
      name=name, value=descriptor(value), equal=descriptor(value == namespace[name]),
      metatable=descriptor(getmetatable(value)),
    }
  end
  return {
    values=values, metatable=descriptor(getmetatable(namespace)),
    mutate_namespace=capture(function() namespace.FORGED=true end),
    mutate_value=capture(function() namespace[names[1]].FORGED=true end),
  }
end

local function run()
  local god, nexus = mux.world.object(1), mux.world.object(0)
  local state = nexus:state('parity')
  state:set('z', 'bytes\0\255')
  state:set('a', true)
  state:set_many({middle=7, toggle=false})
  local room = mux.world.create_object({type=mux.world.types.ROOM,name='Parity Room',zone=nexus})
  local thing = mux.world.create_object(setmetatable({}, {__index={
    type=mux.world.types.THING,name='Parity Thing',location=room,
  }}))
  local exit = mux.world.create_object({type=mux.world.types.EXIT,name='Parity Exit',location=room,destination=nexus})
  thing:set_description(42)
  thing:set_internal_description('internal')
  thing:set_zone(nexus)
  thing:set_affiliation(god)
  thing:set_lua_parent('default_exit.lua')
  local lua_parent = returns(thing:lua_parent())
  local clear_lua_parent = returns(thing:set_lua_parent(nil))
  local flags = thing:flags()
  local had_dark = flags:has(mux.world.flags.DARK)
  local add_dark = flags:add(mux.world.flags.DARK)
  local add_dark_again = flags:add(mux.world.flags.DARK)
  local remove_dark = flags:remove(mux.world.flags.DARK)
  local powers = thing:powers()
  local had_idle = powers:has(mux.world.powers.IDLE)
  local add_idle = powers:add(mux.world.powers.IDLE)
  local add_idle_again = powers:add(mux.world.powers.IDLE)
  local remove_idle = powers:remove(mux.world.powers.IDLE)
  local teleport = returns(mux.world.teleport_object({object=thing,destination=nexus}))
  local teleported_location = returns(thing:location())
  thing:set_home(room)
  local lock_passes = returns(mux.world.lock_passes({
    object=nexus,enactor=god,lock=mux.world.locks.TAKE,
  }))
  mux.world.pemit(god, 'LUA_PARITY_OUTPUT_BEGIN')
  local pemit = returns(mux.world.pemit(god, 'paired world message'))
  local pemit_number = returns(mux.world.pemit(god, 42))
  local pemit_styled = returns(mux.world.pemit(god, mux.text.markup('**paired styled**')))
  local doomed = mux.world.create_object({
    type=mux.world.types.THING,name='Parity Doomed',location=nexus,
  })
  local destroy = returns(mux.world.destroy_object(doomed, setmetatable({}, {__index={override=false}})))
  local stale = capture(function() return doomed:name() end)
  local direct_metamethods = {
    object_tostring = {
      omitted=capture(function() return god.__tostring() end),
      nil_self=capture(function() return god.__tostring(nil) end),
      false_self=capture(function() return god.__tostring(false) end),
      foreign_self=capture(function() return god.__tostring(state) end),
      extra=returns(god.__tostring(god, false)),
      stale=capture(function() return doomed.__tostring(doomed) end),
    },
    object_equal = {
      omitted=capture(function() return god.__eq() end),
      nil_left=capture(function() return god.__eq(nil, god) end),
      nil_right=capture(function() return god.__eq(god, nil) end),
      false_right=capture(function() return god.__eq(god, false) end),
      foreign_right=capture(function() return god.__eq(god, state) end),
      extra=returns(god.__eq(god, mux.world.object(1), false)),
      stale_self=returns(doomed.__eq(doomed, doomed)),
    },
    state_tostring = {
      omitted=capture(function() return state.__tostring() end),
      nil_self=capture(function() return state.__tostring(nil) end),
      false_self=capture(function() return state.__tostring(false) end),
      foreign_self=capture(function() return state.__tostring(god) end),
      extra=returns(state.__tostring(state, false)),
    },
    flags_tostring = {
      omitted=capture(function() return flags.__tostring() end),
      nil_self=capture(function() return flags.__tostring(nil) end),
      false_self=capture(function() return flags.__tostring(false) end),
      foreign_self=capture(function() return flags.__tostring(powers) end),
      extra=returns(flags.__tostring(flags, false)),
    },
    powers_tostring = {
      omitted=capture(function() return powers.__tostring() end),
      nil_self=capture(function() return powers.__tostring(nil) end),
      false_self=capture(function() return powers.__tostring(false) end),
      foreign_self=capture(function() return powers.__tostring(flags) end),
      extra=returns(powers.__tostring(powers, false)),
    },
  }
  return json({
    object = {
      identity = descriptor(god == mux.world.object('1')),
      text = descriptor(tostring(god)), direct_text = returns(god.__tostring(god)),
      direct_equal = returns(god.__eq(god, mux.world.object(1))),
      name = returns(god:name()), type = returns(god:type()), dbref = returns(god:dbref()),
    },
    state = {
      text = descriptor(tostring(state)), get_absent = returns(state:get('absent')),
      get_default = returns(state:get('absent', false)), get_binary = returns(state:get('z')),
      has = returns(state:has('a')), keys = descriptor(state:keys()), entries = descriptor(state:entries()),
      many = descriptor(state:get_many({'z','middle','missing'})),
      delete_present = returns(state:delete('middle')), delete_absent = returns(state:delete('middle')),
      errors = {
        omitted_value = capture(function() return state:set('omitted') end),
        bad_value = capture(function() return state:set('bad', {}) end),
        bad_key = capture(function() return state:get('bad\0key') end),
      },
    },
    created = {
      room = returns(room:dbref(), room:type(), room:zone()),
      thing = returns(thing:dbref(), thing:type(), thing:location(), thing:home(), thing:description(), thing:internal_description(), thing:zone(), thing:affiliation()),
      exit = returns(exit:dbref(), exit:type(), exit:destination()),
      contents = ids(room:contents()), things = ids(room:contents({types={mux.world.types.THING}})),
      list = ids(mux.world.list_objects({in_zone=nexus})),
      lua_parent=lua_parent, clear_lua_parent=clear_lua_parent,
      teleport=teleport, teleported_location=teleported_location,
      lock_passes=lock_passes, pemit=pemit, pemit_number=pemit_number,
      pemit_styled=pemit_styled, destroy=destroy, stale=stale,
      errors={
        create_non_table=capture(function() return mux.world.create_object(false) end),
        create_missing_location=capture(function() return mux.world.create_object({type=mux.world.types.THING,name='x'}) end),
        teleport_nil=capture(function() return mux.world.teleport_object(nil) end),
        destroy_options=capture(function() return mux.world.destroy_object(nexus, false) end),
        list_unknown=capture(function() return mux.world.list_objects({unknown=true}) end),
        pemit_omitted=capture(function() return mux.world.pemit() end),
        pemit_bad_both=capture(function() return mux.world.pemit(false, false) end),
        pemit_bad_both_epcall=capture_error_pcall(function() return mux.world.pemit(false, false) end),
        pemit_nul=capture(function() return mux.world.pemit(god, 'bad\0message') end),
        pemit_binary=capture(function() return mux.world.pemit(god, 'bad\255message') end),
      },
    },
    flags = {
      text = descriptor(tostring(flags)), had_dark=descriptor(had_dark), add=descriptor(add_dark),
      add_again=descriptor(add_dark_again), remove=descriptor(remove_dark), list=descriptor(flags:list()),
      wrong = capture(function() return flags:has(mux.world.powers.IDLE) end),
    },
    powers = {
      text=descriptor(tostring(powers)), had_idle=descriptor(had_idle), add=descriptor(add_idle),
      add_again=descriptor(add_idle_again), remove=descriptor(remove_idle), list=descriptor(powers:list()),
      wrong=capture(function() return powers:has(mux.world.flags.DARK) end),
    },
    direct_metamethods = direct_metamethods,
    constants = {
      room=descriptor(mux.world.types.ROOM), dark=descriptor(mux.world.flags.DARK),
      idle=descriptor(mux.world.powers.IDLE), take=descriptor(mux.world.locks.TAKE),
      type_number=capture(function() return mux.world.types[12] end),
      flag_number=capture(function() return mux.world.flags[123] end),
      types=catalogue(mux.world.types, {'ROOM','THING','EXIT','PLAYER'}),
      access=catalogue(mux.world.access, {'PUBLIC','WIZARD','GOD'}),
      flags=catalogue(mux.world.flags, {
        'ANSI','AUDIBLE','AUDITORIUM','BLIND','CONNECTED','DARK','FLOATING','GAGGED',
        'GOING','HALTED','IN_CHARACTER','LIGHT','MONITOR','NO_COMMAND','SAFE','SUSPECT',
        'TRANSPARENT','WIZARD','ZOMBIE',
      }),
      powers=catalogue(mux.world.powers, {'IDLE'}),
      locks=catalogue(mux.world.locks, {
        'MATCH','TRAVERSE','TAKE','USE','DROP','GIVE','RECEIVE','ENTER','LEAVE',
        'TELEPORT','TELEPORT_OUT','LINK','SET_HOME','SPEAK','CHANNEL_JOIN',
        'CHANNEL_TRANSMIT','CHANNEL_RECEIVE','IDENTIFY_BUILDING',
      }),
    },
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
