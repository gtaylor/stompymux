-- Differential probe: btech.unit administrative setters (scalar chassis values,
-- identity fields, armor patches, technology groups, and typed-constant gates).
--
-- Live units require a BTech special registration, which only the native
-- @btech command performs (no Lua binding registers units in either
-- binary). The setup prelude below creates the fixture objects through a
-- probe command and registers the unit before the measured pages run:
-- PARITY_SETUP: luaparity0
-- PARITY_SETUP: @btech/register Parity Admin Unit=UNIT

local function quote(value)
  return '"' .. value:gsub('[%z\1-\31\\"]', function(byte)
    local escapes = { ['\\'] = '\\\\', ['"'] = '\\"', ['\n'] = '\\n',
      ['\r'] = '\\r', ['\t'] = '\\t' }
    return escapes[byte] or string.format('\\u%04x', string.byte(byte))
  end) .. '"'
end

local function json(value)
  if value == nil then return 'null' end
  if type(value) == 'boolean' or type(value) == 'number' then return tostring(value) end
  if type(value) == 'string' then return quote(value) end
  local out = {}
  if #value > 0 then
    for index = 1, #value do out[#out + 1] = json(value[index]) end
    return '[' .. table.concat(out, ',') .. ']'
  end
  local keys = {}
  for key in pairs(value) do keys[#keys + 1] = key end
  table.sort(keys)
  for _, key in ipairs(keys) do out[#out + 1] = quote(key) .. ':' .. json(value[key]) end
  return '{' .. table.concat(out, ',') .. '}'
end

local function pack(...) return { n = select('#', ...), ... } end

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
    result.bytes = value:gsub('.', function(byte) return string.format('%02x', string.byte(byte)) end)
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

local function capture(function_value, ...)
  local result = pack(pcall(function_value, ...))
  local row = { ok = result[1], return_count = result.n - 1 }
  if result[1] then
    row.values = {}
    for index = 2, result.n do row.values[index - 1] = descriptor(result[index]) end
  else
    row.error = descriptor(result[2])
  end
  return row
end

local state = {}

local function unit()
  return state.unit
end

local unit_api = btech.unit
local sections = unit_api.sections

local function armor_pair(unit_value, section)
  local ok, armor = pcall(unit_api.armor, unit_value, section)
  if not ok then return { error = tostring(armor) } end
  return {
    armor = { armor.armor.current, armor.armor.original },
    internal = { armor.internal.current, armor.internal.original },
    rear = { armor.rear_armor.current, armor.rear_armor.original },
  }
end

local function technology_rows(unit_value)
  local ok, technologies = pcall(unit_api.technologies, unit_value)
  if not ok then return { error = tostring(technologies) } end
  local rows = {}
  for index, technology in ipairs(technologies) do
    rows[index] = {
      code = tostring(technology.code),
      name = technology.name,
      group = technology.group,
      source = technology.source,
    }
  end
  return rows
end

local function page_scalars_and_identity()
  local u = unit()
  local meta_patch = setmetatable({ internal = 3 }, { __index = { armor = 7 } })
  return {
    load = capture(unit_api.load_template, u, 'PARITY-PROBE'),
    scalars = {
      tonnage = capture(unit_api.set_tonnage, u, 90),
      max_speed = capture(unit_api.set_max_speed, u, 6),
      jump_speed = capture(unit_api.set_jump_speed, u, 2),
      heat_sinks = capture(unit_api.set_heat_sinks, u, 15),
      heat_sinks_float = capture(unit_api.set_heat_sinks, u, 15.0),
      lrs = capture(unit_api.set_long_range_sensor_range, u, 30),
      tactical = capture(unit_api.set_tactical_range, u, 9),
      scan = capture(unit_api.set_scan_range, u, 4),
      radio_quality = capture(unit_api.set_radio_quality, u, 3),
      radio_range = capture(unit_api.set_radio_range, u, 400),
      cargo = capture(unit_api.set_cargo_capacity, u, 12, 50),
      cargo_extra_arguments = capture(unit_api.set_cargo_capacity, u, 1, 2, 'ignored'),
      max_speed_extra_arguments = capture(unit_api.set_max_speed, u, 5, 'ignored'),
    },
    identity = {
      display = capture(unit_api.set_display_name, u, 'Admin Probe'),
      display_read = capture(unit_api.display_name, u),
      markings = capture(unit_api.set_markings, u, 'stripe'),
      markings_read = capture(unit_api.markings, u),
      preferred = capture(unit_api.set_preferred_id, u, 'ab'),
      preferred_read = capture(unit_api.preferred_id, u),
      pilot = capture(unit_api.set_assigned_pilot, u, mux.world.object(1)),
      pilot_read_dbref = capture(function()
        return unit_api.assigned_pilot(u):dbref()
      end),
      display_clear = capture(unit_api.set_display_name, u, nil),
      display_read_cleared = capture(unit_api.display_name, u),
      markings_clear = capture(unit_api.set_markings, u, nil),
      markings_read_cleared = capture(unit_api.markings, u),
      preferred_clear = capture(unit_api.set_preferred_id, u, nil),
      preferred_read_cleared = capture(unit_api.preferred_id, u),
      pilot_clear = capture(unit_api.set_assigned_pilot, u, nil),
      pilot_read_cleared = capture(unit_api.assigned_pilot, u),
      display_extra_arguments = capture(unit_api.set_display_name, u, 'Kept', false, 'extra'),
      display_120_bytes = capture(unit_api.set_display_name, u, string.rep('n', 120)),
      display_120_bytes_read = capture(unit_api.display_name, u),
    },
    identity_errors = {
      display_false = capture(unit_api.set_display_name, u, false),
      display_empty = capture(unit_api.set_display_name, u, ''),
      display_number = capture(unit_api.set_display_name, u, 42),
      display_121_bytes = capture(unit_api.set_display_name, u, string.rep('n', 121)),
      markings_false = capture(unit_api.set_markings, u, false),
      markings_16384_bytes = capture(unit_api.set_markings, u, string.rep('m', 16384)),
      preferred_empty = capture(unit_api.set_preferred_id, u, ''),
      preferred_one_letter = capture(unit_api.set_preferred_id, u, 'a'),
      preferred_three_letters = capture(unit_api.set_preferred_id, u, 'abc'),
      preferred_digits = capture(unit_api.set_preferred_id, u, '1a'),
      preferred_false = capture(unit_api.set_preferred_id, u, false),
      display_arity = capture(unit_api.set_display_name),
      markings_arity = capture(unit_api.set_markings, u),
      preferred_arity = capture(unit_api.set_preferred_id),
      pilot_arity = capture(unit_api.set_assigned_pilot, u),
      pilot_not_player = capture(unit_api.set_assigned_pilot, u, u),
      pilot_false = capture(unit_api.set_assigned_pilot, u, false),
      pilot_wrong_kind_unit = capture(unit_api.set_assigned_pilot, mux.world.object(1), mux.world.object(1)),
    },
    armor = {
      full_patch = capture(unit_api.set_armor, u, sections.HEAD,
        { armor = 7, internal = 3, rear_armor = 1 }),
      full_patch_read = armor_pair(u, sections.HEAD),
      partial_patch = capture(unit_api.set_armor, u, sections.HEAD, { armor = 9 }),
      partial_patch_read = armor_pair(u, sections.HEAD),
      metatable_patch = capture(unit_api.set_armor, u, sections.HEAD, meta_patch),
      metatable_patch_read = armor_pair(u, sections.HEAD),
      boundary_255 = capture(unit_api.set_armor, u, sections.HEAD,
        { armor = 255, internal = 255, rear_armor = 255 }),
      boundary_255_read = armor_pair(u, sections.HEAD),
    },
    armor_errors = {
      value_256 = capture(unit_api.set_armor, u, sections.HEAD, { armor = 256 }),
      value_negative = capture(unit_api.set_armor, u, sections.HEAD, { internal = -1 }),
      value_fraction = capture(unit_api.set_armor, u, sections.HEAD, { armor = 1.5 }),
      value_nan = capture(unit_api.set_armor, u, sections.HEAD, { rear_armor = 0 / 0 }),
      value_false = capture(unit_api.set_armor, u, sections.HEAD, { armor = false }),
      value_string = capture(unit_api.set_armor, u, sections.HEAD, { armor = '7' }),
      empty_patch = capture(unit_api.set_armor, u, sections.HEAD, {}),
      unknown_field = capture(unit_api.set_armor, u, sections.HEAD, { armor = 1, zzz = 0 }),
      patch_false = capture(unit_api.set_armor, u, sections.HEAD, false),
      patch_omitted = capture(unit_api.set_armor, u, sections.HEAD),
      patch_number = capture(unit_api.set_armor, u, sections.HEAD, 7),
      section_nil = capture(unit_api.set_armor, u, nil, { armor = 1 }),
      section_omitted = capture(unit_api.set_armor, u),
      section_string = capture(unit_api.set_armor, u, 'HEAD', { armor = 1 }),
      section_wrong_catalog = capture(unit_api.set_armor, u, unit_api.technology.ECM, { armor = 1 }),
    },
    __next_commands = { 'luaparity2' },
  }
end

local function page_typing_and_boundaries()
  local u = unit()
  return {
    typing = {
      infantry_on_mech = capture(unit_api.add_technology, u, unit_api.technology.SWARM_ATTACK),
      set_naval = capture(unit_api.set_unit_type, u, unit_api.types.NAVAL),
      naval_head_armor = capture(unit_api.set_armor, u, sections.HEAD, { armor = 1 }),
      naval_left_side = capture(unit_api.set_armor, u, sections.LEFT_SIDE, { armor = 6 }),
      naval_left_side_read = armor_pair(u, sections.LEFT_SIDE),
      naval_turret_read = armor_pair(u, sections.TURRET),
      movement_hull = capture(unit_api.set_movement_type, u, unit_api.movement_types.HULL),
      set_battlesuit = capture(unit_api.set_unit_type, u, unit_api.types.BATTLESUIT),
      battlesuit_movement = capture(function()
        return armor_pair(u, sections.SUIT_1)
      end),
      infantry_add = capture(unit_api.add_technology, u, unit_api.technology.SWARM_ATTACK),
      technologies_battlesuit = technology_rows(u),
      clear_infantry = capture(unit_api.clear_technologies, u, unit_api.technology_groups.INFANTRY),
      technologies_infantry_cleared = technology_rows(u),
      clear_unit_group = capture(unit_api.clear_technologies, u, unit_api.technology_groups.UNIT),
      technologies_unit_cleared = technology_rows(u),
      restore_mech = capture(unit_api.set_unit_type, u, unit_api.types.MECH),
      restore_biped = capture(unit_api.set_movement_type, u, unit_api.movement_types.BIPED),
      technologies_restored = technology_rows(u),
    },
    typed_constant_errors = {
      movement_given_type = capture(unit_api.set_movement_type, u, unit_api.types.MECH),
      movement_number = capture(unit_api.set_movement_type, u, 1),
      movement_string = capture(unit_api.set_movement_type, u, 'TRACK'),
      unit_type_given_movement = capture(unit_api.set_unit_type, u, unit_api.movement_types.TRACK),
      unit_type_number = capture(unit_api.set_unit_type, u, 5),
      technology_given_section = capture(unit_api.add_technology, u, sections.HEAD),
      technology_string = capture(unit_api.add_technology, u, 'ECM'),
      technology_given_group = capture(unit_api.remove_technology, u, unit_api.technology_groups.ALL),
      technology_omitted = capture(unit_api.add_technology, u),
      group_given_technology = capture(unit_api.clear_technologies, u, unit_api.technology.ECM),
      group_number = capture(unit_api.clear_technologies, u, 1),
      group_omitted = capture(unit_api.clear_technologies, u),
    },
    boundaries = {
      tons_low = capture(unit_api.set_tonnage, u, 0),
      tons_edge_low = capture(unit_api.set_tonnage, u, 1),
      tons_edge_high = capture(unit_api.set_tonnage, u, 2097151),
      tons_high = capture(unit_api.set_tonnage, u, 2097152),
      tons_fraction = capture(unit_api.set_tonnage, u, 1.5),
      tons_false = capture(unit_api.set_tonnage, u, false),
      tons_string = capture(unit_api.set_tonnage, u, '5'),
      tons_omitted = capture(unit_api.set_tonnage, u),
      speed_negative = capture(unit_api.set_max_speed, u, -1),
      speed_edge_high = capture(unit_api.set_jump_speed, u, 10000),
      speed_high = capture(unit_api.set_jump_speed, u, 10000.5),
      speed_nan = capture(unit_api.set_max_speed, u, 0 / 0),
      speed_huge = capture(unit_api.set_max_speed, u, math.huge),
      speed_string = capture(unit_api.set_max_speed, u, '5'),
      heat_edge_high = capture(unit_api.set_heat_sinks, u, 127),
      heat_high = capture(unit_api.set_heat_sinks, u, 128),
      heat_fraction = capture(unit_api.set_heat_sinks, u, 12.5),
      heat_false = capture(unit_api.set_heat_sinks, u, false),
      lrs_edge = capture(unit_api.set_long_range_sensor_range, u, 127),
      lrs_high = capture(unit_api.set_long_range_sensor_range, u, 128),
      lrs_negative = capture(unit_api.set_tactical_range, u, -1),
      tac_high = capture(unit_api.set_tactical_range, u, 128),
      scan_high = capture(unit_api.set_scan_range, u, 128),
      scan_string = capture(unit_api.set_scan_range, u, '5'),
      quality_edge_low = capture(unit_api.set_radio_quality, u, 1),
      quality_edge_high = capture(unit_api.set_radio_quality, u, 5),
      quality_low = capture(unit_api.set_radio_quality, u, 0),
      quality_high = capture(unit_api.set_radio_quality, u, 6),
      quality_fraction = capture(unit_api.set_radio_quality, u, 2.5),
      radio_range_edge = capture(unit_api.set_radio_range, u, 32767),
      radio_range_high = capture(unit_api.set_radio_range, u, 32768),
      radio_range_negative = capture(unit_api.set_radio_range, u, -1),
      cargo_space_edge = capture(unit_api.set_cargo_capacity, u, 5000, 100),
      cargo_space_high = capture(unit_api.set_cargo_capacity, u, 5001, 10),
      cargo_tons_edge_high = capture(unit_api.set_cargo_capacity, u, 10, 100),
      cargo_tons_low = capture(unit_api.set_cargo_capacity, u, 10, 0),
      cargo_tons_high = capture(unit_api.set_cargo_capacity, u, 10, 101),
      cargo_space_false = capture(unit_api.set_cargo_capacity, u, false, 1),
      cargo_tons_omitted = capture(unit_api.set_cargo_capacity, u, 10),
    },
    handles = {
      wrong_kind_object = capture(unit_api.set_max_speed, mux.world.object(1), 5),
      wrong_kind_identity = capture(unit_api.set_display_name, mux.world.object(1), 'x'),
      nil_unit = capture(unit_api.set_tonnage, nil, 5),
      false_unit = capture(unit_api.set_max_speed, false, 5),
      garbage_dbref = capture(unit_api.set_tonnage, 99999, 5),
      number_unit = capture(function()
        return unit_api.set_tonnage(state.unit:dbref(), 95)
      end),
    },
  }
end

local function emit(target, payload)
  local size = 3000
  local count = math.max(1, math.ceil(#payload / size))
  for index = 1, count do
    local chunk = payload:sub(((index - 1) * size) + 1, index * size)
    mux.world.pemit(target, 'LUA_PARITY:' .. index .. '/' .. count .. ':' .. chunk)
  end
end

local pages = {
  page_scalars_and_identity,
  page_typing_and_boundaries,
}

local commands = {{
  name = 'lua-parity-probe-prepare', permission = 'everyone', pattern = '^luaparity0$',
  handler = function(ctx)
    -- The @btech registration setup line resolves this unit by name from the
    -- enactor's location, matching native MUX name lookup.
    state.unit = mux.world.create_object({
      type = mux.world.types.THING, name = 'Parity Admin Unit',
      location = mux.world.object(0),
    })
    return true
  end,
}}
for number = 1, #pages do
  local page_number = number
  commands[#commands + 1] = {
    name = 'lua-parity-probe-' .. page_number, permission = 'everyone',
    pattern = '^luaparity' .. ((page_number == 1 and '') or tostring(page_number)) .. '$',
    handler = function(ctx)
      emit(ctx.enactor, json(pages[page_number]()))
      return true
    end,
  }
end
return { commands = commands }
