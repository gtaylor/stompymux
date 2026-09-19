-- Differential probe: btech.template contract edges over the seeded PARITY
-- templates. Complements template_inspection.lua (which covers the pristine
-- projections) with argument coercion, arity, and rejection surfaces.
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
    if value.code and type(value.code) ~= 'userdata' then result.code = value.code end
    if value.message then result.message = value.message end
    if value.detail ~= nil then result.detail_has_argument = value.detail.argument end
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

local function run()
  local player = mux.world.object(1)
  local catalog = {}
  for _, reference in ipairs({
    'PARITY-PROBE', 'PARITY-TECH', 'PARITY-GROUND', 'PARITY-VTOL',
    'PARITY-NAVAL', 'PARITY-SPHEROID', 'PARITY-AERO', 'PARITY-AERODYNE',
    'PARITY-BSUIT', 'PARITY-MW',
  }) do
    catalog[#catalog + 1] = {
      reference = reference,
      exists = call(btech.template.exists, reference),
      base_cost = call(btech.template.base_cost, reference),
      battle_value = call(btech.template.battle_value, reference),
      engine = call(btech.template.engine, reference),
      payload = call(btech.template.payload, reference),
      installed_parts = call(btech.template.installed_parts, reference),
      technologies = call(btech.template.technologies, reference),
      weapons = call(btech.template.weapons, reference),
    }
  end
  local sections = {
    mech = btech.unit.sections.HEAD, turret = btech.unit.sections.TURRET,
    rotor = btech.unit.sections.ROTOR, naval = btech.unit.sections.LEFT_SIDE,
  }
  local shaped = {
    lookup = {
      missing_exists = call(btech.template.exists, 'PARITY-MISSING'),
      shipped_generic_exists = call(btech.template.exists, 'ABS-3L'),
      shipped_generic_engine = call(btech.template.engine, 'ABS-3L'),
    },
    probe = {
      armor_all = call(btech.template.armor, 'PARITY-PROBE'),
      armor_head = call(btech.template.armor, 'PARITY-PROBE', sections.mech),
      criticals_head = call(btech.template.critical_slots, 'PARITY-PROBE', sections.mech),
      weapons_head = call(btech.template.weapons, 'PARITY-PROBE', sections.mech),
      ground_turret_armor = call(btech.template.armor, 'PARITY-GROUND', sections.turret),
      ground_turret_criticals = call(btech.template.critical_slots, 'PARITY-GROUND', sections.turret),
      vtol_rotor_weapons = call(btech.template.weapons, 'PARITY-VTOL', sections.rotor),
      naval_side_armor = call(btech.template.armor, 'PARITY-NAVAL', sections.naval),
    },
    arity = {
      battle_value_none = call(btech.template.battle_value),
      battle_value_extra = call(btech.template.battle_value, 'PARITY-PROBE', 'extra'),
    },
    reference_errors = {
      number = call(btech.template.exists, 42),
      boolean = call(btech.template.exists, false),
      omitted = call(btech.template.exists),
      empty = call(btech.template.exists, ''),
      parent = call(btech.template.exists, '../PARITY-PROBE'),
      slash = call(btech.template.exists, 'stock/PARITY-PROBE'),
      backslash = call(btech.template.exists, 'PARITY-PROBE\\x'),
      engine_number = call(btech.template.engine, 42),
      cost_missing = call(btech.template.base_cost, 'PARITY-MISSING'),
      payload_missing = call(btech.template.payload, 'PARITY-MISSING'),
    },
    section_errors = {
      armor_false = call(btech.template.armor, 'PARITY-PROBE', false),
      armor_number = call(btech.template.armor, 'PARITY-PROBE', 42),
      armor_wrong_namespace = call(btech.template.armor, 'PARITY-PROBE',
        btech.unit.fire_modes.DESTROYED),
      armor_invalid_for_unit = call(btech.template.armor, 'PARITY-PROBE', sections.turret),
      criticals_omitted = call(btech.template.critical_slots, 'PARITY-PROBE'),
      criticals_nil = call(btech.template.critical_slots, 'PARITY-PROBE', nil),
      criticals_invalid = call(btech.template.critical_slots, 'PARITY-PROBE', sections.turret),
      weapons_invalid = call(btech.template.weapons, 'PARITY-PROBE', sections.turret),
    },
    display = {
      status = call(btech.template.show_status, 'PARITY-PROBE', player),
      weapon_specs = call(btech.template.show_weapon_specs, 'PARITY-PROBE', player),
      critical_status = call(btech.template.show_critical_status, 'PARITY-PROBE', player,
        sections.mech),
      status_room = call(btech.template.show_status, 'PARITY-PROBE', mux.world.object(0)),
      status_number = call(btech.template.show_status, 'PARITY-PROBE', 0),
      criticals_missing_section = call(btech.template.show_critical_status, 'PARITY-PROBE',
        player),
      criticals_nil_section = call(btech.template.show_critical_status, 'PARITY-PROBE', player,
        nil),
      criticals_invalid_section = call(btech.template.show_critical_status, 'PARITY-PROBE',
        player, sections.turret),
      status_missing_template = call(btech.template.show_status, 'PARITY-MISSING', player),
    },
    extra_arguments = {
      exists = call(btech.template.exists, 'PARITY-PROBE', 'extra'),
      engine = call(btech.template.engine, 'PARITY-PROBE', 'extra'),
      base_cost = call(btech.template.base_cost, 'PARITY-PROBE', 'extra'),
      payload = call(btech.template.payload, 'PARITY-PROBE', 'extra'),
      installed_parts = call(btech.template.installed_parts, 'PARITY-PROBE', 'extra'),
      technologies = call(btech.template.technologies, 'PARITY-PROBE', 'extra'),
      armor_nil_section = call(btech.template.armor, 'PARITY-PROBE', nil, 'extra'),
      weapons_nil_section = call(btech.template.weapons, 'PARITY-PROBE', nil, 'extra'),
      show_status = call(btech.template.show_status, 'PARITY-PROBE', player, 'extra'),
    },
  }
  return json({ catalog = catalog, shaped = shaped })
end

local function emit(target, payload)
  local size = 3000
  local count = math.max(1, math.ceil(#payload / size))
  for index = 1, count do
    local chunk = payload:sub(((index - 1) * size) + 1, index * size)
    mux.world.pemit(target, 'LUA_PARITY:' .. index .. '/' .. count .. ':' .. chunk)
  end
end

local commands = {{
    name = 'lua-parity-probe', permission = 'everyone', pattern = '^luaparity$',
    handler = function(ctx)
      mux.world.pemit(ctx.enactor, 'LUA_PARITY_OUTPUT_BEGIN')
      emit(ctx.enactor, run())
      return true
    end,
  }}
return {commands = commands}
