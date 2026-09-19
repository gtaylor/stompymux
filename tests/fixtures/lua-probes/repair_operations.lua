-- Differential probe: btech.repair.apply immediate operations and package shape.
--
-- Live units require a BTech special registration, which only the native
-- @btech command performs. The setup prelude below creates two fixture things
-- and registers both as MECH before any measured page runs: one loads the
-- branded PARITY-PROBE template inside page one, the other stays pristine for
-- the raw-unit page.
-- PARITY_SETUP: luaparity0
-- PARITY_SETUP: @btech/register Parity Repair Unit=MECH
-- PARITY_SETUP: @btech/register Parity Raw Unit=MECH

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

local function returns(...)
  local result = pack(...)
  local row = { count = result.n }
  for index = 1, result.n do row[index] = descriptor(result[index]) end
  return row
end

local state = {}

local sections = btech.unit.sections
local operations = btech.repair.operations

local function unit()
  return state.unit
end

local function raw_unit()
  return state.raw_unit
end

local function armor_pair(unit_value, section)
  local ok, armor = pcall(btech.unit.armor, unit_value, section)
  if not ok then return { error = tostring(armor) } end
  return {
    armor = { armor.armor.current, armor.armor.original },
    internal = { armor.internal.current, armor.internal.original },
    rear = { armor.rear_armor.current, armor.rear_armor.original },
  }
end

local function critical_row(unit_value, section, slot)
  local ok, criticals = pcall(btech.unit.critical_slots, unit_value, section)
  if not ok then return { error = tostring(criticals) } end
  local item = criticals[slot]
  if not item then return { absent = true } end
  local row = {
    kind = item.kind,
    operational = item.operational,
    temporary_failure = item.temporary_failure,
    auxiliary_data = item.auxiliary_data,
    fire_modes = {},
  }
  for index, mode in ipairs(item.fire_modes) do row.fire_modes[index] = tostring(mode) end
  return row
end

local function weapon_number_in_section(unit_value, section)
  local ok, weapons = pcall(btech.unit.weapons, unit_value)
  if not ok then return nil end
  for _, weapon in ipairs(weapons) do
    if weapon.section == section then return weapon.number end
  end
  return nil
end

local function page_loaded()
  -- Typed constant namespaces resolve every shipped name; tostring gives the
  -- canonical per-name text both binaries must agree on.
  local operation_names = {
    'REATTACH', 'REPAIR_PART', 'REPAIR_WEAPON_TEMPORARY', 'REPAIR_ENHANCEMENT',
    'REPAIR_FOCUS', 'REPAIR_CRYSTAL', 'REPAIR_BARREL', 'REPAIR_AMMO_FEED',
    'REPAIR_RANGING', 'REPAIR_AMMO_MOUNT', 'REPLACE_WEAPON', 'RELOAD',
    'REPAIR_ARMOR', 'REPAIR_REAR_ARMOR', 'REPAIR_INTERNAL', 'DETACH',
    'SCRAP_PART', 'SCRAP_WEAPON', 'UNLOAD', 'RESEAL', 'REPLACE_SUIT',
  }
  local operation_text = {}
  for index, name in ipairs(operation_names) do
    operation_text[index] = tostring(operations[name])
  end
  return {
    operation_names = operation_text,
    load = capture(btech.unit.load_template, unit(), 'PARITY-PROBE'),
    armor_head_loaded = armor_pair(unit(), sections.HEAD),
    armor = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 4 }),
    armor_head_after = armor_pair(unit(), sections.HEAD),
    armor_zero = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 0 }),
    armor_max = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 255 }),
    internal = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_INTERNAL, section = sections.HEAD, value = 2 }),
    armor_head_internal = armor_pair(unit(), sections.HEAD),
    rear_torso = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_REAR_ARMOR, section = sections.CENTER_TORSO, value = 5 }),
    rear_center_after = armor_pair(unit(), sections.CENTER_TORSO),
    part_head = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_PART, section = sections.HEAD, slot = 1 }),
    is_fixable = returns(btech.repair.is_fixable(unit())),
    errors = {
      request_omitted = capture(btech.repair.apply, unit()),
      request_nil = capture(btech.repair.apply, unit(), nil),
      request_boolean = capture(btech.repair.apply, unit(), true),
      operation_omitted = capture(btech.repair.apply, unit(), { section = sections.HEAD }),
      operation_reload = capture(btech.repair.apply, unit(),
        { operation = operations.RELOAD, section = sections.HEAD, value = 1 }),
      operation_replace_weapon = capture(btech.repair.apply, unit(),
        { operation = operations.REPLACE_WEAPON, section = sections.HEAD, value = 1 }),
      operation_detach = capture(btech.repair.apply, unit(),
        { operation = operations.DETACH, section = sections.HEAD }),
      operation_scrap_part = capture(btech.repair.apply, unit(),
        { operation = operations.SCRAP_PART, section = sections.HEAD, slot = 1 }),
      operation_reseal = capture(btech.repair.apply, unit(),
        { operation = operations.RESEAL, section = sections.HEAD }),
      operation_replace_suit = capture(btech.repair.apply, unit(),
        { operation = operations.REPLACE_SUIT, section = sections.HEAD }),
      unknown_field = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 1, extra = true }),
      rear_on_head = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_REAR_ARMOR, section = sections.HEAD, value = 1 }),
      value_over = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 256 }),
      value_fraction = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 1.5 }),
      value_negative = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = -1 }),
      value_missing = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_ARMOR, section = sections.HEAD }),
      slot_zero = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_PART, section = sections.HEAD, slot = 0 }),
      slot_over_head = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_PART, section = sections.HEAD, slot = 7 }),
      slot_over_torso = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_PART, section = sections.CENTER_TORSO, slot = 13 }),
      section_turret = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_ARMOR, section = sections.TURRET, value = 1 }),
      section_wrong_catalog = capture(btech.repair.apply, unit(),
        { operation = operations.REPAIR_ARMOR, section = btech.unit.types.MECH, value = 1 }),
      section_player = capture(btech.repair.apply, mux.world.object(1),
        { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 1 }),
      unit_room = capture(btech.repair.apply, mux.world.object(0),
        { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 1 }),
    },
    __next_commands = { 'luaparity2' },
  }
end

local function page_restoration()
  -- Damage the left-arm PPC deterministically, then repair its criticals.
  local weapon = weapon_number_in_section(unit(), sections.LEFT_ARM)
  local fire_modes = btech.unit.fire_modes
  local mark = capture(btech.unit.set_weapon_modes, unit(), weapon,
    { fire_modes = { fire_modes.DESTROYED, fire_modes.DISABLED, fire_modes.BROKEN,
      fire_modes.DAMAGED, fire_modes.ONE_SHOT_USED } })
  return {
    weapon_number = descriptor(weapon),
    mark = mark,
    damaged_critical = critical_row(unit(), sections.LEFT_ARM, 2),
    repair_part_weapon = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_PART, section = sections.LEFT_ARM, slot = 2 }),
    repaired_critical = critical_row(unit(), sections.LEFT_ARM, 2),
    empty_slot_head = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_PART, section = sections.HEAD, slot = 4 }),
    -- Exhaust a leg completely; C mech_re_attach restores only destroyed sections.
    zero_leg_armor = capture(btech.unit.set_armor, unit(), sections.LEFT_LEG, 0),
    zero_leg_internal = capture(btech.repair.apply, unit(),
      { operation = operations.REPAIR_INTERNAL, section = sections.LEFT_LEG, value = 0 }),
    leg_destroyed = armor_pair(unit(), sections.LEFT_LEG),
    is_fixable_leg = returns(btech.repair.is_fixable(unit())),
    reattach_leg = capture(btech.repair.apply, unit(),
      { operation = operations.REATTACH, section = sections.LEFT_LEG }),
    leg_reattached = armor_pair(unit(), sections.LEFT_LEG),
    reattach_undamaged = capture(btech.repair.apply, unit(),
      { operation = operations.REATTACH, section = sections.HEAD }),
    __next_commands = { 'luaparity3' },
  }
end

local function page_raw()
  return {
    raw_is_fixable = returns(btech.repair.is_fixable(raw_unit())),
    raw_armor_head = armor_pair(raw_unit(), sections.HEAD),
    raw_armor = capture(btech.repair.apply, raw_unit(),
      { operation = operations.REPAIR_ARMOR, section = sections.HEAD, value = 2 }),
    raw_internal = capture(btech.repair.apply, raw_unit(),
      { operation = operations.REPAIR_INTERNAL, section = sections.HEAD, value = 1 }),
    raw_part = capture(btech.repair.apply, raw_unit(),
      { operation = operations.REPAIR_PART, section = sections.HEAD, slot = 1 }),
    raw_part_empty = capture(btech.repair.apply, raw_unit(),
      { operation = operations.REPAIR_PART, section = sections.HEAD, slot = 3 }),
    raw_reattach = capture(btech.repair.apply, raw_unit(),
      { operation = operations.REATTACH, section = sections.HEAD }),
    raw_armor_after = armor_pair(raw_unit(), sections.HEAD),
    raw_rear = capture(btech.repair.apply, raw_unit(),
      { operation = operations.REPAIR_REAR_ARMOR, section = sections.HEAD, value = 1 }),
    raw_slot_over = capture(btech.repair.apply, raw_unit(),
      { operation = operations.REPAIR_PART, section = sections.HEAD, slot = 7 }),
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

local pages = { page_loaded, page_restoration, page_raw }

local commands = {{
  name = 'lua-parity-probe-prepare', permission = 'everyone', pattern = '^luaparity0$',
  handler = function(ctx)
    state.unit = mux.world.create_object({
      type = mux.world.types.THING, name = 'Parity Repair Unit',
      location = mux.world.object(0),
    })
    state.raw_unit = mux.world.create_object({
      type = mux.world.types.THING, name = 'Parity Raw Unit',
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
