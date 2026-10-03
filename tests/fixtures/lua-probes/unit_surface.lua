-- Differential probe: btech.unit read-only inspection surface.
--
-- The harness prelude registers three live MECH special objects, and the probe
-- loads a parity template onto each before projecting every inspection getter.
-- PARITY-NAVAL stays unloaded here: Rust's unit-construction dispatch does not
-- accept Naval-class templates yet (C template_load.c loads every class), so
-- that variant is blocked on the load_template owner.
--
-- PARITY_SETUP: @create Parity Surface Mech
-- PARITY_SETUP: @btech/register Parity Surface Mech=MECH
-- PARITY_SETUP: @create Parity Surface Ground
-- PARITY_SETUP: @btech/register Parity Surface Ground=MECH
-- PARITY_SETUP: @create Parity Surface VTOL
-- PARITY_SETUP: @btech/register Parity Surface VTOL=MECH

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
  if type(value) == 'number' then return quote(string.format('%.17g', value)) end
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

local function named(god, name)
  for _, thing in ipairs(god:contents()) do
    if thing:name() == name then return thing end
  end
  return nil
end

local function armor_record(row)
  return {
    section = row.section and tostring(row.section) or nil,
    armor_current = row.armor.current, armor_original = row.armor.original,
    internal_current = row.internal.current, internal_original = row.internal.original,
    rear_current = row.rear_armor.current, rear_original = row.rear_armor.original,
  }
end

local function weapon_rows(rows)
  local out = {}
  for index, row in ipairs(rows) do
    out[index] = {
      number = row.number, section = tostring(row.section),
      first_slot = row.first_slot, part_id = row.part.id,
      slot_count = row.slot_count,
      recycle = row.recycle, recycle_time = row.recycle_time,
      operational = row.operational,
    }
  end
  return out
end

local function critical_rows(rows)
  local out = {}
  for index, row in ipairs(rows) do
    out[index] = {
      section = tostring(row.section), slot = row.slot, kind = row.kind,
      has_part = row.part ~= nil, part_id = row.part and row.part.id or nil,
      operational = row.operational, temporary_failure = row.temporary_failure,
      auxiliary_data = row.auxiliary_data,
      ammunition_rounds = row.ammunition and row.ammunition.rounds or nil,
      ammunition_capacity = row.ammunition and row.ammunition.capacity or nil,
      fire_modes = #row.fire_modes, ammunition_modes = #row.ammunition_modes,
    }
  end
  return out
end

local function inventory_rows(rows)
  local out = {}
  for index, row in ipairs(rows) do
    out[index] = {
      part_id = row.part.id, quantity = row.quantity,
    }
  end
  return out
end

local function technology_rows(rows)
  local out = {}
  for index, row in ipairs(rows) do
    out[index] = {
      code = tostring(row.code), name = row.name, group = row.group, source = row.source,
    }
  end
  return out
end

local function radio_rows(rows)
  local out = {}
  for index, row in ipairs(rows) do
    local modes = {}
    for mode = 1, #row.modes do modes[mode] = row.modes[mode] end
    out[index] = {
      channel = row.channel, frequency = row.frequency, title = row.title, modes = modes,
    }
  end
  return out
end

-- Project every inspection getter over one loaded unit.
local function projections(unit, section)
  local armor = btech.unit.armor(unit)
  local armor_section = btech.unit.armor(unit, section)
  local engine = btech.unit.engine(unit)
  local bv = btech.unit.battle_value(unit)
  local radios = btech.unit.radio_channels(unit)
  return {
    armor = armor_record(armor),
    armor_section = armor_record(armor_section),
    critical_slots = critical_rows(btech.unit.critical_slots(unit, section)),
    weapons = weapon_rows(btech.unit.weapons(unit)),
    weapons_section = weapon_rows(btech.unit.weapons(unit, section)),
    tic_weapons = weapon_rows(btech.unit.tic_weapons(unit, 0)),
    installed_parts = inventory_rows(btech.unit.installed_parts(unit)),
    payload = inventory_rows(btech.unit.payload(unit)),
    technologies = technology_rows(btech.unit.technologies(unit)),
    radio_channels = radio_rows(radios),
    engine = { rating = engine.rating, suspension_factor = engine.suspension_factor },
    battle_value = { total = bv.total, offensive = bv.offensive, defensive = bv.defensive },
    effective_max_speed = btech.unit.effective_max_speed(unit),
    effective_max_speed_kph = btech.unit.effective_max_speed_kph(unit),
    section_condition = btech.unit.section_condition(unit, section),
    preferred_id = btech.unit.preferred_id(unit),
    markings = btech.unit.markings(unit),
    display_name = btech.unit.display_name(unit),
    assigned_pilot = btech.unit.assigned_pilot(unit) ~= nil,
    arity = {
      armor = select('#', btech.unit.armor(unit)),
      preferred_id = select('#', btech.unit.preferred_id(unit)),
      assigned_pilot = select('#', btech.unit.assigned_pilot(unit)),
      battle_value = select('#', btech.unit.battle_value(unit)),
    },
  }
end

-- Argument-matrix row for one callable against the shared invalid handles.
local function edges(name, state)
  local function_value = btech.unit[name]
  return {
    name = name,
    none = call(function_value),
    nil_handle = call(function_value, nil),
    boolean = call(function_value, false),
    text = call(function_value, '#nope'),
    table = call(function_value, {}),
    dbref = call(function_value, 999999),
    string_dbref = call(function_value, '3'),
    player = call(function_value, state.god),
    thing = call(function_value, state.unregistered),
    room = call(function_value, state.room),
    extra_thing = call(function_value, state.unregistered, 'extra', 'more'),
  }
end

local GETTERS = {
  'armor', 'critical_slots', 'weapons', 'radio_channels', 'engine', 'battle_value',
  'payload', 'installed_parts', 'technologies', 'preferred_id', 'markings',
  'display_name', 'assigned_pilot', 'effective_max_speed', 'effective_max_speed_kph',
  'section_condition', 'tic_weapons',
}

local function shared(god)
  local zone = mux.world.create_object({
    type = mux.world.types.ROOM, name = 'Parity Unit Surface Zone',
  })
  return {
    god = god,
    room = mux.world.object(0),
    unregistered = mux.world.create_object({
      type = mux.world.types.THING, name = 'Parity Unregistered Unit', location = zone,
    }),
  }
end

local function page_one()
  local god = mux.world.object(1)
  local state = shared(god)
  local rows = {}
  for index = 1, 9 do rows[#rows + 1] = edges(GETTERS[index], state) end
  return json({
    edges = rows,
    __next_commands = { 'luaparity2', 'luaparity3', 'luaparity4' },
  })
end

local function page_two()
  local god = mux.world.object(1)
  local state = shared(god)
  local rows = {}
  for index = 10, 17 do rows[#rows + 1] = edges(GETTERS[index], state) end
  local arity = {
    battle_value_none = call(btech.unit.battle_value),
    battle_value_extra = call(btech.unit.battle_value, state.unregistered, nil),
    preferred_id_none = call(btech.unit.preferred_id),
    markings_none = call(btech.unit.markings),
    assigned_pilot_none = call(btech.unit.assigned_pilot),
    display_name_none = call(btech.unit.display_name),
  }
  return json({ edges = rows, arity = arity })
end

local function loaded(name, reference)
  local god = mux.world.object(1)
  local unit = named(god, name)
  return {
    find = unit ~= nil,
    load = call(btech.unit.load_template, unit, reference),
  }
end

local function page_three()
  local god = mux.world.object(1)
  local state = shared(god)
  local unit = named(god, 'Parity Surface Mech')
  local setup = loaded('Parity Surface Mech', 'PARITY-PROBE')
  local surface = {
    setup = setup,
    mech = projections(unit, btech.unit.sections.LEFT_ARM),
    errors = {
      critical_slots_none = call(btech.unit.critical_slots, unit),
      critical_slots_nil = call(btech.unit.critical_slots, unit, nil),
      critical_slots_turret = call(btech.unit.critical_slots, unit, btech.unit.sections.TURRET),
      section_condition_none = call(btech.unit.section_condition, unit),
      section_condition_turret = call(btech.unit.section_condition, unit, btech.unit.sections.TURRET),
      weapons_turret = call(btech.unit.weapons, unit, btech.unit.sections.TURRET),
      armor_number = call(btech.unit.armor, unit, 42),
      armor_string = call(btech.unit.armor, unit, 'HEAD'),
      tic_text = call(btech.unit.tic_weapons, unit, 'x'),
      tic_high = call(btech.unit.tic_weapons, unit, 4),
      tic_fraction = call(btech.unit.tic_weapons, unit, 1.5),
      tic_nil = call(btech.unit.tic_weapons, unit, nil),
      battle_value_extra = call(btech.unit.battle_value, unit, nil),
      armor_player = call(btech.unit.armor, state.god),
      armor_unregistered = call(btech.unit.armor, state.unregistered),
      load_missing = call(btech.unit.load_template, unit, 'PARITY-MISSING'),
      load_stock = call(btech.unit.load_template, unit, 'jr7-d'),
    },
  }
  return json({ mech = surface })
end

local function page_four()
  local ground_unit = named(mux.world.object(1), 'Parity Surface Ground')
  local vtol_unit = named(mux.world.object(1), 'Parity Surface VTOL')
  return json({
    ground = {
      setup = loaded('Parity Surface Ground', 'PARITY-GROUND'),
      unit = projections(ground_unit, btech.unit.sections.TURRET),
    },
    vtol = {
      setup = loaded('Parity Surface VTOL', 'PARITY-VTOL'),
      unit = projections(vtol_unit, btech.unit.sections.FRONT_SIDE),
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

local PAGES = { page_one, page_two, page_three, page_four }

local commands = {{
  name = 'lua-parity-probe', permission = 'everyone', pattern = '^luaparity$',
  handler = function(ctx)
    mux.world.pemit(ctx.enactor, 'LUA_PARITY_OUTPUT_BEGIN')
    emit(ctx.enactor, PAGES[1]())
    return true
  end,
}}
for number = 2, #PAGES do
  local page_number = number
  commands[#commands + 1] = {
    name = 'lua-parity-probe-' .. page_number, permission = 'everyone',
    pattern = '^luaparity' .. page_number .. '$',
    handler = function(ctx) emit(ctx.enactor, PAGES[page_number]()); return true end,
  }
end
return {commands = commands}
