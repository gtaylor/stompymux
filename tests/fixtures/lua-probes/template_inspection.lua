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
    result.bytes = (value:gsub('.', function(byte)
      return string.format('%02x', string.byte(byte))
    end))
  elseif kind == 'userdata' then
    result.text = tostring(value)
  elseif kind == 'table' then
    seen = seen or {}
    if seen[value] then result.cycle = true; return result end
    seen[value] = true
    local ordered = {}
    for key, item in pairs(value) do
      ordered[#ordered + 1] = {
        sort = type(key) .. ':' .. tostring(key), key = key, value = item,
      }
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

local function pack(...) return { n = select('#', ...), ... } end

local function call(function_value, ...)
  local result = pack(pcall(function_value, ...))
  local response = { ok = result[1], return_count = result.n - 1 }
  if not result[1] then
    response.error = descriptor(result[2])
  else
    response.returns = {}
    for index = 2, result.n do response.returns[index - 1] = descriptor(result[index]) end
  end
  return response
end

local function inspect(reference, section, player)
  return {
    exists = call(btech.template.exists, reference),
    base_cost = call(btech.template.base_cost, reference),
    raw = {
      armor = call(btech.template.armor, reference),
      section_armor = call(btech.template.armor, reference, section),
      engine = call(btech.template.engine, reference),
      battle_value = call(btech.template.battle_value, reference),
      weapons = call(btech.template.weapons, reference),
      criticals = call(btech.template.critical_slots, reference, section),
      installed_parts = call(btech.template.installed_parts, reference),
      payload = call(btech.template.payload, reference),
      technologies = call(btech.template.technologies, reference),
    },
    display_returns = {
      status = call(btech.template.show_status, reference, player),
      weapons = call(btech.template.show_weapon_specs, reference, player),
      criticals = call(btech.template.show_critical_status, reference, player, section),
    },
  }
end

local function inspect_raw(reference, section)
  return {
    exists = call(btech.template.exists, reference),
    armor = call(btech.template.armor, reference),
    section_armor = call(btech.template.armor, reference, section),
    engine = call(btech.template.engine, reference),
    battle_value = call(btech.template.battle_value, reference),
    base_cost = call(btech.template.base_cost, reference),
    weapons = call(btech.template.weapons, reference),
    criticals = call(btech.template.critical_slots, reference, section),
    installed_parts = call(btech.template.installed_parts, reference),
    payload = call(btech.template.payload, reference),
    technologies = call(btech.template.technologies, reference),
  }
end

local function run()
  local player = mux.world.object(1)
  return json({
    __capture_output = true,
    fixtures = {
      mech = inspect('PARITY-PROBE', btech.unit.sections.HEAD, player),
      technology = inspect('PARITY-TECH', btech.unit.sections.LEFT_ARM, player),
      ground = inspect('PARITY-GROUND', btech.unit.sections.TURRET, player),
      vtol = inspect('PARITY-VTOL', btech.unit.sections.ROTOR, player),
      naval = { raw = inspect_raw('PARITY-NAVAL', btech.unit.sections.LEFT_SIDE) },
      spheroid = { raw = inspect_raw('PARITY-SPHEROID',
        btech.unit.sections.FRONT_RIGHT_SIDE) },
      aero = { raw = inspect_raw('PARITY-AERO', btech.unit.sections.NOSE) },
      mechwarrior = { raw = inspect_raw('PARITY-MW', btech.unit.sections.LEFT_ARM) },
      aerodyne = { raw = inspect_raw('PARITY-AERODYNE',
        btech.unit.sections.RIGHT_WING) },
      battlesuit = { raw = inspect_raw('PARITY-BSUIT', btech.unit.sections.SUIT_1) },
    },
    errors = {
      missing = call(btech.template.engine, 'PARITY-MISSING'),
      invalid_reference = call(btech.template.engine, '../PARITY-PROBE'),
      invalid_section = call(btech.template.armor, 'PARITY-PROBE',
        btech.unit.sections.TURRET),
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
