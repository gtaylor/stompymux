-- Differential probe: btech.unit mutation and construction operations.
--
-- Live units require a BTech special registration, which only the native
-- @btech command performs (no Lua binding registers units in either
-- binary). The setup prelude below creates the fixture objects through a
-- probe command and registers the unit before the measured pages run:
-- PARITY_SETUP: luaparity0
-- PARITY_SETUP: @btech/register Parity Mutation Unit=MECH

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

local sections = btech.unit.sections

local function armor_pair(unit_value, section)
  local ok, armor = pcall(btech.unit.armor, unit_value, section)
  if not ok then return { error = tostring(armor) } end
  return {
    armor = { armor.armor.current, armor.armor.original },
    internal = { armor.internal.current, armor.internal.original },
    rear = { armor.rear_armor.current, armor.rear_armor.original },
  }
end

local function weapon_rows(unit_value)
  local ok, weapons = pcall(btech.unit.weapons, unit_value)
  if not ok then return { error = tostring(weapons) } end
  local rows = {}
  for index, weapon in ipairs(weapons) do
    rows[index] = {
      number = weapon.number,
      section = tostring(weapon.section),
      first_slot = weapon.first_slot,
      slot_count = weapon.slot_count,
      recycle = weapon.recycle,
      recycle_time = weapon.recycle_time,
      operational = weapon.operational,
    }
  end
  return rows
end

local function technology_rows(unit_value)
  local ok, technologies = pcall(btech.unit.technologies, unit_value)
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

local function critical_row(unit_value, section, slot)
  local ok, criticals = pcall(btech.unit.critical_slots, unit_value, section)
  if not ok then return { error = tostring(criticals) } end
  local item = criticals[slot]
  local row = {
    kind = item.kind,
    part_id = item.part and item.part.id or nil,
    part_brand = item.part and item.part.brand or nil,
    operational = item.operational,
    temporary_failure = item.temporary_failure,
    auxiliary_data = item.auxiliary_data,
    fire_modes = {},
    ammunition_modes = {},
  }
  for index, mode in ipairs(item.fire_modes) do row.fire_modes[index] = tostring(mode) end
  for index, mode in ipairs(item.ammunition_modes) do
    row.ammunition_modes[index] = tostring(mode)
  end
  if item.ammunition then
    row.rounds = item.ammunition.rounds
    row.capacity = item.ammunition.capacity
  end
  return row
end

local function page_lifecycle()
  return {
    registered_raw_weapons = weapon_rows(unit()),
    load = capture(btech.unit.load_template, unit(), 'PARITY-PROBE'),
    armor_head_loaded = armor_pair(unit(), sections.HEAD),
    load_case_insensitive = capture(btech.unit.load_template, unit(), 'parity-probe'),
    exists_before_save = capture(btech.template.exists, 'PARITY-SAVED'),
    save = capture(btech.unit.save_template, unit(), 'PARITY-SAVED'),
    exists_after_save = capture(btech.template.exists, 'PARITY-SAVED'),
    exists_after_save_case = capture(btech.template.exists, 'parity-saved'),
    reload_saved = capture(btech.unit.load_template, unit(), 'PARITY-SAVED'),
    armor_head_saved = armor_pair(unit(), sections.HEAD),
    restore = capture(btech.unit.restore, unit()),
    armor_head_restored = armor_pair(unit(), sections.HEAD),
    extra_arguments = {
      load = capture(btech.unit.load_template, unit(), 'PARITY-SAVED', 'ignored'),
      save = capture(btech.unit.save_template, unit(), 'PARITY-SAVED-EXTRA', 'ignored'),
    },
    errors = {
      load_missing_template = capture(btech.unit.load_template, unit(), 'PARITY-MISSING'),
      load_omitted = capture(btech.unit.load_template, unit()),
      load_nil = capture(btech.unit.load_template, unit(), nil),
      load_empty = capture(btech.unit.load_template, unit(), ''),
      load_number = capture(btech.unit.load_template, unit(), 42),
      load_boolean = capture(btech.unit.load_template, unit(), false),
      load_table = capture(btech.unit.load_template, unit(), {}),
      load_path = capture(btech.unit.load_template, unit(), '../PARITY'),
      load_slash = capture(btech.unit.load_template, unit(), 'sub/PARITY'),
      load_wrong_kind = capture(btech.unit.load_template, mux.world.object(1), 'PARITY-PROBE'),
      save_empty = capture(btech.unit.save_template, unit(), ''),
      save_omitted = capture(btech.unit.save_template, unit()),
      save_long = capture(btech.unit.save_template, unit(), string.rep('x', 256)),
      save_number = capture(btech.unit.save_template, unit(), 42),
      save_path = capture(btech.unit.save_template, unit(), '../bad'),
      restore_unregistered = capture(btech.unit.restore, mux.world.object(0)),
    },
    __next_commands = { 'luaparity2', 'luaparity3' },
  }
end

local function page_equipment()
  local request = {
    part = 'Agra.IS.PPC',
    section = sections.LEFT_TORSO,
    slots = { 2, 3, 4 },
  }
  return {
    weapons_loaded = weapon_rows(unit()),
    install = capture(btech.unit.install_weapon, unit(), request),
    weapons_installed = weapon_rows(unit()),
    installed_brand_zero = critical_row(unit(), sections.LEFT_TORSO, 2),
    install_errors = {
      duplicate_slots = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO, slots = { 2, 2, 3 },
      }),
      short_slots = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO, slots = { 2 },
      }),
      empty_slots = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO, slots = {},
      }),
      long_slots = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO,
        slots = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13 },
      }),
      slot_zero = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO, slots = { 0, 3, 4 },
      }),
      slot_fraction = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO, slots = { 1.5, 3, 4 },
      }),
      slot_string = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO, slots = { '2', 3, 4 },
      }),
      slots_omitted = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO,
      }),
      part_boolean = capture(btech.unit.install_weapon, unit(), {
        part = true, section = sections.LEFT_TORSO, slots = { 5, 6, 7 },
      }),
      part_unregistered_packed = capture(btech.unit.install_weapon, unit(), {
        part = 115, section = sections.LEFT_TORSO, slots = { 5, 6, 7 },
      }),
      part_unregistered_record = capture(btech.unit.install_weapon, unit(), {
        part = { id = 406, brand = 0 }, section = sections.LEFT_TORSO, slots = { 5, 6, 7 },
      }),
      section_wrong_catalog = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = btech.unit.types.MECH, slots = { 5, 6, 7 },
      }),
      section_wrong_unit = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.TURRET, slots = { 1 },
      }),
      section_omitted = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', slots = { 5, 6, 7 },
      }),
      request_not_table = capture(btech.unit.install_weapon, unit(), false),
      request_omitted = capture(btech.unit.install_weapon, unit()),
      unknown_field = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO, slots = { 5, 6, 7 }, extra = 1,
      }),
      rear_facing_string = capture(btech.unit.install_weapon, unit(), {
        part = 'Agra.IS.PPC', section = sections.LEFT_TORSO, slots = { 5, 6, 7 },
        rear_facing = 'yes',
      }),
    },
    configure = capture(btech.unit.configure_ammunition, unit(), {
      weapon = 'Telos.IS.SRM-4', section = sections.RIGHT_TORSO, slot = 1,
      half_ton = true, ammunition_modes = { btech.unit.ammunition_modes.INFERNO },
    }),
    configured_slot = critical_row(unit(), sections.RIGHT_TORSO, 1),
    restock = capture(btech.unit.restock_ammunition, unit(), sections.RIGHT_TORSO, 1),
    restocked_slot = critical_row(unit(), sections.RIGHT_TORSO, 1),
    ammunition_errors = {
      weapon_not_ammunition = capture(btech.unit.configure_ammunition, unit(), {
        weapon = 'Agra.IS.PPC', section = sections.RIGHT_TORSO, slot = 2,
      }),
      weapon_unregistered = capture(btech.unit.configure_ammunition, unit(), {
        weapon = 'Not.A.Part', section = sections.RIGHT_TORSO, slot = 2,
      }),
      slot_zero = capture(btech.unit.configure_ammunition, unit(), {
        weapon = 'Telos.IS.SRM-4', section = sections.RIGHT_TORSO, slot = 0,
      }),
      slot_fraction = capture(btech.unit.configure_ammunition, unit(), {
        weapon = 'Telos.IS.SRM-4', section = sections.RIGHT_TORSO, slot = 1.5,
      }),
      modes_not_table = capture(btech.unit.configure_ammunition, unit(), {
        weapon = 'Telos.IS.SRM-4', section = sections.RIGHT_TORSO, slot = 2,
        ammunition_modes = 'Inferno',
      }),
      modes_string_entry = capture(btech.unit.configure_ammunition, unit(), {
        weapon = 'Telos.IS.SRM-4', section = sections.RIGHT_TORSO, slot = 2,
        ammunition_modes = { 'Inferno' },
      }),
      half_ton_string = capture(btech.unit.configure_ammunition, unit(), {
        weapon = 'Telos.IS.SRM-4', section = sections.RIGHT_TORSO, slot = 2,
        half_ton = 'yes',
      }),
      restock_weapon_slot = capture(btech.unit.restock_ammunition, unit(),
        sections.LEFT_TORSO, 2),
      restock_slot_range = capture(btech.unit.restock_ammunition, unit(),
        sections.RIGHT_TORSO, 13),
      restock_slot_fraction = capture(btech.unit.restock_ammunition, unit(),
        sections.RIGHT_TORSO, 1.5),
      restock_slot_omitted = capture(btech.unit.restock_ammunition, unit(),
        sections.RIGHT_TORSO),
      restock_section_wrong_catalog = capture(btech.unit.restock_ammunition, unit(),
        btech.unit.types.MECH, 1),
    },
    set_modes = capture(btech.unit.set_weapon_modes, unit(), 0, {
      fire_modes = { btech.unit.fire_modes.HOTLOAD },
      ammunition_modes = { btech.unit.ammunition_modes.PRECISION },
    }),
    modes_slot = critical_row(unit(), sections.LEFT_ARM, 1),
    mode_errors = {
      unmounted = capture(btech.unit.set_weapon_modes, unit(), 9, {}),
      negative_number = capture(btech.unit.set_weapon_modes, unit(), -1, {}),
      fraction_number = capture(btech.unit.set_weapon_modes, unit(), 0.5, {}),
      string_number = capture(btech.unit.set_weapon_modes, unit(), '0', {}),
      omitted_number = capture(btech.unit.set_weapon_modes, unit()),
      modes_not_table = capture(btech.unit.set_weapon_modes, unit(), 0, 'modes'),
      fire_string_entry = capture(btech.unit.set_weapon_modes, unit(), 0, {
        fire_modes = { 'Hotload' },
      }),
      fire_wrong_catalog = capture(btech.unit.set_weapon_modes, unit(), 0, {
        fire_modes = { btech.unit.ammunition_modes.INFERNO },
      }),
      unknown_field = capture(btech.unit.set_weapon_modes, unit(), 0, { extra = true }),
    },
    special_clear = capture(btech.unit.install_special, unit(), {
      section = sections.HEAD, slot = 6,
    }),
    special_clear_with_data = capture(btech.unit.install_special, unit(), {
      section = sections.LEFT_TORSO, slot = 5, auxiliary_data = 7,
    }),
    special_errors = {
      weapon_part = capture(btech.unit.install_special, unit(), {
        part = 'Agra.IS.PPC', section = sections.HEAD, slot = 6,
      }),
      unregistered_part = capture(btech.unit.install_special, unit(), {
        part = { id = 406, brand = 0 }, section = sections.HEAD, slot = 6,
      }),
      named_part = capture(btech.unit.install_special, unit(), {
        part = 'Agra.IS.FerroFibrous', section = sections.HEAD, slot = 6,
      }),
      part_string = capture(btech.unit.install_special, unit(), {
        part = 'nope', section = sections.HEAD, slot = 6,
      }),
      slot_range = capture(btech.unit.install_special, unit(), {
        section = sections.HEAD, slot = 13,
      }),
      auxiliary_string = capture(btech.unit.install_special, unit(), {
        section = sections.HEAD, slot = 6, auxiliary_data = 'seven',
      }),
      auxiliary_fraction = capture(btech.unit.install_special, unit(), {
        section = sections.HEAD, slot = 6, auxiliary_data = 1.5,
      }),
      unknown_field = capture(btech.unit.install_special, unit(), {
        section = sections.HEAD, slot = 6, extra = 1,
      }),
    },
    reset = capture(btech.unit.reset_critical_slots, unit()),
    weapons_after_reset = weapon_rows(unit()),
    reload_after_reset = capture(btech.unit.load_template, unit(), 'PARITY-SAVED'),
    weapons_after_reload = weapon_rows(unit()),
  }
end

local function page_damage_and_technology()
  return {
    reload = capture(btech.unit.load_template, unit(), 'PARITY-SAVED'),
    armor_left_arm = armor_pair(unit(), sections.LEFT_ARM),
    armor_left_torso = armor_pair(unit(), sections.LEFT_TORSO),
    hit_front = capture(btech.unit.apply_damage, unit(), {
      amount = 3, cluster_size = 1, direction_code = 0,
    }),
    armor_left_arm_after = armor_pair(unit(), sections.LEFT_ARM),
    hit_clustered = capture(btech.unit.apply_damage, unit(), {
      amount = 3, cluster_size = 2, direction_code = 0,
    }),
    armor_left_arm_clustered = armor_pair(unit(), sections.LEFT_ARM),
    hit_rear = capture(btech.unit.apply_damage, unit(), {
      amount = 1, cluster_size = 1, direction_code = 10,
    }),
    armor_left_torso_rear = armor_pair(unit(), sections.LEFT_TORSO),
    hit_critical = capture(btech.unit.apply_damage, unit(), {
      amount = 1, cluster_size = 1, direction_code = 0, force_critical = true,
    }),
    armor_left_arm_critical = armor_pair(unit(), sections.LEFT_ARM),
    section_condition = {
      operational = capture(btech.unit.section_condition, unit(), sections.LEFT_ARM),
    },
    damage_errors = {
      amount_zero = capture(btech.unit.apply_damage, unit(),
        { amount = 0, cluster_size = 1, direction_code = 0 }),
      amount_over = capture(btech.unit.apply_damage, unit(),
        { amount = 1001, cluster_size = 1, direction_code = 0 }),
      amount_negative = capture(btech.unit.apply_damage, unit(),
        { amount = -1, cluster_size = 1, direction_code = 0 }),
      amount_fraction = capture(btech.unit.apply_damage, unit(),
        { amount = 2.5, cluster_size = 1, direction_code = 0 }),
      amount_string = capture(btech.unit.apply_damage, unit(),
        { amount = '2', cluster_size = 1, direction_code = 0 }),
      amount_omitted = capture(btech.unit.apply_damage, unit(),
        { cluster_size = 1, direction_code = 0 }),
      amount_nil = capture(btech.unit.apply_damage, unit(),
        { amount = nil, cluster_size = 1, direction_code = 0 }),
      amount_false = capture(btech.unit.apply_damage, unit(),
        { amount = false, cluster_size = 1, direction_code = 0 }),
      cluster_zero = capture(btech.unit.apply_damage, unit(),
        { amount = 1, cluster_size = 0, direction_code = 0 }),
      direction_over = capture(btech.unit.apply_damage, unit(),
        { amount = 1, cluster_size = 1, direction_code = 22 }),
      direction_negative = capture(btech.unit.apply_damage, unit(),
        { amount = 1, cluster_size = 1, direction_code = -1 }),
      direction_fraction = capture(btech.unit.apply_damage, unit(),
        { amount = 1, cluster_size = 1, direction_code = 0.5 }),
      force_critical_string = capture(btech.unit.apply_damage, unit(),
        { amount = 1, cluster_size = 1, direction_code = 0, force_critical = 'yes' }),
      unit_message_number = capture(btech.unit.apply_damage, unit(),
        { amount = 1, cluster_size = 1, direction_code = 0, unit_message = 7 }),
      map_message_table = capture(btech.unit.apply_damage, unit(),
        { amount = 1, cluster_size = 1, direction_code = 0, map_message = {} }),
      unknown_field = capture(btech.unit.apply_damage, unit(),
        { amount = 1, cluster_size = 1, direction_code = 0, extra = true }),
      request_not_table = capture(btech.unit.apply_damage, unit(), false),
      request_omitted = capture(btech.unit.apply_damage, unit()),
      wrong_kind_unit = capture(btech.unit.apply_damage, mux.world.object(1),
        { amount = 1, cluster_size = 1, direction_code = 0 }),
    },
    technologies_initial = technology_rows(unit()),
    add = capture(btech.unit.add_technology, unit(), btech.unit.technology.ECM),
    technologies_added = technology_rows(unit()),
    remove = capture(btech.unit.remove_technology, unit(), btech.unit.technology.ECM),
    technologies_removed = technology_rows(unit()),
    clear_all = capture(btech.unit.clear_technologies, unit(),
      btech.unit.technology_groups.ALL),
    technologies_cleared = technology_rows(unit()),
    technology_errors = {
      add_infantry = capture(btech.unit.add_technology, unit(),
        btech.unit.technology.SWARM_ATTACK),
      add_string = capture(btech.unit.add_technology, unit(), 'ECM'),
      add_wrong_catalog = capture(btech.unit.add_technology, unit(),
        btech.unit.technology_groups.ALL),
      add_omitted = capture(btech.unit.add_technology, unit()),
      remove_string = capture(btech.unit.remove_technology, unit(), 'ECM'),
      clear_wrong_constant = capture(btech.unit.clear_technologies, unit(),
        btech.unit.technology.ECM),
      clear_omitted = capture(btech.unit.clear_technologies, unit()),
      wrong_kind_unit = capture(btech.unit.add_technology, mux.world.object(1),
        btech.unit.technology.ECM),
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
  page_lifecycle,
  page_equipment,
  page_damage_and_technology,
}

local commands = {{
  name = 'lua-parity-probe-prepare', permission = 'everyone', pattern = '^luaparity0$',
  handler = function(ctx)
    -- The @btech registration setup line resolves this unit by name from the
    -- enactor's location, matching native MUX name lookup.
    state.unit = mux.world.create_object({
      type = mux.world.types.THING, name = 'Parity Mutation Unit',
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
