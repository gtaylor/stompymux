-- Differential probe: btech.player configuration surface on the seeded GOD
-- player plus btech.system.units_in_zone over a freshly built zone.
local function quote(value)
  return '"' .. value:gsub('[%z\1-\31\\"]', function(byte)
    local escapes = { ['\\'] = '\\\\', ['"'] = '\\"', ['\n'] = '\\n',
      ['\r'] = '\\r', ['\t'] = '\\t' }
    return escapes[byte] or string.format('\\u%04x', string.byte(byte))
  end) .. '"'
end

local function json(value)
  if value == nil then return 'null' end
  if type(value) == 'boolean' then return tostring(value) end
  if type(value) == 'number' then return quote(tostring(value)) end
  if type(value) == 'string' then return quote(value) end
  local array = #value > 0
  local out = {}
  if array then
    for index = 1, #value do out[#out + 1] = json(value[index]) end
    return '[' .. table.concat(out, ',') .. ']'
  end
  local keys = {}
  for key in pairs(value) do keys[#keys + 1] = key end
  table.sort(keys)
  for _, key in ipairs(keys) do out[#out + 1] = quote(key) .. ':' .. json(value[key]) end
  return '{' .. table.concat(out, ',') .. '}'
end

local function descriptor(value)
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
    result.bytes = (value:gsub('.', function(byte)
      return string.format('%02x', string.byte(byte))
    end))
  elseif kind == 'userdata' then
    result.text = tostring(value)
  elseif kind == 'table' then
    local keys = {}
    for key in pairs(value) do keys[#keys + 1] = type(key) .. ':' .. tostring(key) end
    table.sort(keys)
    result.keys = keys
    if value.code ~= nil and type(value.code) ~= 'userdata' then result.code = value.code end
    if value.message ~= nil then result.message = value.message end
    if value.detail ~= nil and value.detail.argument ~= nil then
      result.detail_has_argument = value.detail.argument
    end
  end
  return result
end

local function call(function_value, ...)
  local results = { pcall(function_value, ...) }
  local response = { ok = results[1], count = #results - 1 }
  if not results[1] then
    response.error = descriptor(results[2])
  else
    response.returns = {}
    for index = 2, #results do response.returns[index - 1] = descriptor(results[index]) end
  end
  return response
end

local function table_of(function_value, ...)
  local results = { pcall(function_value, ...) }
  if not results[1] then
    return { ok = false, error = descriptor(results[2]) }
  end
  if results[2] == nil then
    return { ok = true, count = #results - 1, value = 'null' }
  end
  local entries = {}
  local keys = {}
  for key in pairs(results[2]) do keys[#keys + 1] = key end
  table.sort(keys)
  for _, key in ipairs(keys) do
    entries[#entries + 1] = quote(key) .. ':' .. json(results[2][key])
  end
  return { ok = true, count = #results - 1, value = '{' .. table.concat(entries, ',') .. '}' }
end

local function units(function_value, zone, ...)
  local results = { pcall(function_value, zone, ...) }
  if not results[1] then
    return { ok = false, error = descriptor(results[2]) }
  end
  local rows = {}
  if results[2] ~= nil then
    for index, unit in ipairs(results[2]) do
      rows[#rows + 1] = { index = index, text = tostring(unit) }
    end
  end
  return { ok = true, count = #results - 1, units = rows, total = #results[2] }
end

local function run()
  local god = mux.world.object(1)
  local preferences = {
    tactical_height = 12, tactical_width = 30, lrs_height = 20,
    include_dead = true, include_shutdown = false, include_enemies = true,
    include_allies = false, include_target = true, buildings = 'follow_brief',
  }
  local loadout = {
    armor = { head = 2, torso = 8, hands = 1, feet = 2 },
    right = { weapon = 'PC.Blazer', ammunition = 12 },
    left = { weapon = 'PC.Sword' },
  }
  local player_surface = {
    defaults = table_of(btech.player.ui_preferences, god),
    default_fields = call(btech.player.ui_preferences, god),
    set_preferences = call(btech.player.set_ui_preferences, god, preferences),
    configured = table_of(btech.player.ui_preferences, god),
    clear_preferences = call(btech.player.set_ui_preferences, god, nil),
    cleared = table_of(btech.player.ui_preferences, god),
    template_initial = call(btech.player.mechwarrior_template, god),
    set_template = call(btech.player.set_mechwarrior_template, god, 'PARITY-MW'),
    template_set = call(btech.player.mechwarrior_template, god),
    clear_template = call(btech.player.set_mechwarrior_template, god, nil),
    template_cleared = call(btech.player.mechwarrior_template, god),
    loadout_initial = call(btech.player.loadout, god),
    set_loadout = call(btech.player.set_loadout, god, loadout),
    loadout_after_weapons = table_of(btech.player.loadout, god),
    set_armor_only = call(btech.player.set_loadout, god, { armor = loadout.armor }),
    loadout_set = table_of(btech.player.loadout, god),
    clear_loadout = call(btech.player.set_loadout, god, nil),
    loadout_cleared = call(btech.player.loadout, god),
    extra_arguments = {
      ui_preferences = table_of(btech.player.ui_preferences, god, 'extra'),
      mechwarrior_template = call(btech.player.mechwarrior_template, god, 'extra'),
      loadout = call(btech.player.loadout, god, 'extra'),
      set_ui_preferences = call(btech.player.set_ui_preferences, god, nil, 'extra'),
      set_mechwarrior_template = call(btech.player.set_mechwarrior_template, god, nil, 'extra'),
      set_loadout = call(btech.player.set_loadout, god, nil, 'extra'),
    },
    arity = {
      ui_preferences = call(btech.player.ui_preferences),
      mechwarrior_template = call(btech.player.mechwarrior_template),
      loadout = call(btech.player.loadout),
      set_ui_preferences = call(btech.player.set_ui_preferences, god),
      set_mechwarrior_template = call(btech.player.set_mechwarrior_template, god),
      set_loadout = call(btech.player.set_loadout, god),
    },
    rejections = {
      ui_room = call(btech.player.ui_preferences, mux.world.object(0)),
      ui_number = call(btech.player.ui_preferences, 0),
      ui_text = call(btech.player.ui_preferences, '#nope'),
      set_preferences_number = call(btech.player.set_ui_preferences, god, 42),
      set_preferences_unknown_field = call(btech.player.set_ui_preferences, god, {
        tactical_height = 12, tactical_width = 30, lrs_height = 20,
        include_dead = true, include_shutdown = false, include_enemies = true,
        include_allies = false, include_target = true, buildings = 'exclude',
        extra = 1,
      }),
      set_preferences_bad_height = call(btech.player.set_ui_preferences, god,
        setmetatable({}, { __index = preferences, tactical_height = 4 })),
      set_preferences_bad_buildings = call(btech.player.set_ui_preferences, god,
        setmetatable({}, { __index = preferences, buildings = 'bogus' })),
      set_loadout_number = call(btech.player.set_loadout, god, 42),
      set_loadout_bad_armor = call(btech.player.set_loadout, god,
        { armor = 5, right = loadout.right }),
      set_loadout_missing_weapon = call(btech.player.set_loadout, god,
        { armor = loadout.armor, right = { weapon = 'Missing.Weapon' } }),
      set_loadout_ammo_weapon = call(btech.player.set_loadout, god,
        { armor = loadout.armor, right = { weapon = 'Ammo_IS.SRM-4' } }),
      set_loadout_registry_weapon = call(btech.player.set_loadout, god,
        { armor = loadout.armor, right = { weapon = 'IS.PPC' } }),
      set_loadout_personal_weapon = call(btech.player.set_loadout, god,
        { armor = loadout.armor, right = { weapon = 'PC.Blazer' } }),
      set_loadout_bad_ammunition = call(btech.player.set_loadout, god,
        { armor = loadout.armor, right = { weapon = 'PC.Blazer', ammunition = 256 } }),
      set_template_number = call(btech.player.set_mechwarrior_template, god, 42),
      set_template_empty = call(btech.player.set_mechwarrior_template, god, ''),
      set_template_path = call(btech.player.set_mechwarrior_template, god, '../PARITY-MW'),
      set_template_missing = call(btech.player.set_mechwarrior_template, god, 'PARITY-MISSING'),
      set_template_generic = call(btech.player.set_mechwarrior_template, god, 'ABS-3L'),
    },
  }
  local zone = mux.world.create_object({
    type = mux.world.types.ROOM, name = 'Parity Zone',
  })
  local other = mux.world.create_object({
    type = mux.world.types.ROOM, name = 'Parity Other Zone',
  })
  local first = mux.world.create_object({
    type = mux.world.types.THING, name = 'Parity Unit One', location = zone,
  })
  local plain = mux.world.create_object({
    type = mux.world.types.THING, name = 'Parity Plain Thing', location = zone,
  })
  local second = mux.world.create_object({
    type = mux.world.types.THING, name = 'Parity Unit Two', location = zone,
  })
  local elsewhere = mux.world.create_object({
    type = mux.world.types.THING, name = 'Parity Unit Elsewhere', location = other,
  })
  local system_surface = {
    lag_type = type(btech.system.event_lag()),
    lag_extra_type = type(btech.system.event_lag('ignored', 'more')),
    zone_before = units(btech.system.units_in_zone, zone),
    load_first = call(btech.unit.load_template, first, 'PARITY-PROBE'),
    zone_unzoned = units(btech.system.units_in_zone, zone),
    set_zone_first = call(first.set_zone, first, zone),
    zone_one = units(btech.system.units_in_zone, zone, 'ignored'),
    load_plain = call(btech.unit.load_template, plain, 'PARITY-PROBE'),
    load_second = call(btech.unit.load_template, second, 'PARITY-PROBE'),
    set_zone_plain = call(plain.set_zone, plain, zone),
    set_zone_second = call(second.set_zone, second, zone),
    load_elsewhere = call(btech.unit.load_template, elsewhere, 'PARITY-PROBE'),
    set_zone_elsewhere = call(elsewhere.set_zone, elsewhere, other),
    zone_two = units(btech.system.units_in_zone, zone),
    clear_zone_plain = call(plain.set_zone, plain, nil),
    zone_after_clear = units(btech.system.units_in_zone, zone),
    zone_missing = units(btech.system.units_in_zone, other),
    errors = {
      nil_zone = call(btech.system.units_in_zone, nil),
      number_zone = call(btech.system.units_in_zone, 42),
      text_zone = call(btech.system.units_in_zone, '#nope'),
      player_zone = call(btech.system.units_in_zone, god),
    },
  }
  return json({ player = player_surface, system = system_surface })
end

local function emit(target, payload)
  local size = 3000
  local count = math.max(1, math.ceil(#payload / size))
  for index = 1, count do
    local chunk = payload:sub(((index - 1) * size) + 1, index * size)
    mux.world.pemit(target, 'LUA_PARITY:' .. index .. '/' .. count .. ':' .. chunk)
  end
end

return {
  commands = {{
    name = 'lua-parity-probe', permission = 'everyone', pattern = '^luaparity$',
    handler = function(ctx)
      mux.world.pemit(ctx.enactor, 'LUA_PARITY_OUTPUT_BEGIN')
      emit(ctx.enactor, run())
      return true
    end,
  }},
}
