-- Differential probe: typed constant namespaces across mux and btech.
-- Surveys the 18 constant namespaces (265 constants), recording per namespace
-- the sorted key inventory, per-constant descriptors with equality checks, and
-- pcall-protected lookup/mutation error edges. Names come from the pinned C
-- catalogues; userdata namespaces cannot be enumerated with pairs().

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

-- C argument failures render as "<where>bad argument #N to '<name>' (<detail>)".
-- The wrapper spelling is incidental to the namespace contract, so the audit
-- compares the failure code, argument detail and the wrapped domain text.
local function message_detail(message)
  if type(message) ~= 'string' then return nil end
  return message:match("bad argument #%d+ to '.-' %((.*)%)$") or message
end

-- Record one pcall edge: failure code, structured detail and message text.
local function edge(call)
  local ok, err = pcall(call)
  local row = { ok = ok }
  if not ok and type(err) == 'table' then
    row.error = {
      code = descriptor(err.code),
      detail = descriptor(err.detail),
      message = descriptor(message_detail(err.message)),
    }
  elseif not ok then
    row.error = descriptor(err)
  end
  return row
end

local function resolve(path)
  local value = _G
  for part in path:gmatch('[^.]+') do value = value[part] end
  return value
end

-- Cross-namespace partners keep the world flag first: the C world-flag __eq
-- compares without type checks, so these comparisons return false instead of
-- raising the type errors a lock/access/type/comsys first operand would raise.
local CROSS = {
  btech_types = function(value) return value == btech.unit.types.MECH end,
  btech_movement = function(value)
    return value == btech.unit.movement_types.BIPED
  end,
  to_comsys = function(value) return value == mux.comsys.flags.PUBLIC end,
  from_flags = function(value) return mux.world.flags.CONNECTED == value end,
}

local SPECS = {
  {
    page = 1, path = 'mux.world.flags', cross = 'to_comsys',
    names = { 'ANSI', 'AUDIBLE', 'AUDITORIUM', 'BLIND', 'CONNECTED', 'DARK',
      'FLOATING', 'GAGGED', 'GOING', 'HALTED', 'IN_CHARACTER', 'LIGHT',
      'MONITOR', 'NO_COMMAND', 'SAFE', 'SUSPECT', 'TRANSPARENT', 'WIZARD',
      'ZOMBIE' },
  },
  {
    page = 1, path = 'mux.world.locks', cross = 'from_flags',
    names = { 'MATCH', 'TRAVERSE', 'TAKE', 'USE', 'DROP', 'GIVE', 'RECEIVE',
      'ENTER', 'LEAVE', 'TELEPORT', 'TELEPORT_OUT', 'LINK', 'SET_HOME',
      'SPEAK', 'CHANNEL_JOIN', 'CHANNEL_TRANSMIT', 'CHANNEL_RECEIVE',
      'IDENTIFY_BUILDING' },
  },
  {
    page = 1, path = 'mux.world.powers', cross = 'to_comsys',
    names = { 'IDLE' },
  },
  {
    page = 2, path = 'mux.world.types', cross = 'from_flags',
    names = { 'ROOM', 'THING', 'EXIT', 'PLAYER' },
  },
  {
    page = 2, path = 'mux.world.access', cross = 'from_flags',
    names = { 'PUBLIC', 'WIZARD', 'GOD' },
  },
  {
    page = 2, path = 'mux.comsys.flags', cross = 'from_flags',
    names = { 'PUBLIC', 'LOUD', 'TRANSPARENT' },
  },
  {
    page = 3, path = 'btech.unit.types', cross = 'btech_movement',
    names = { 'MECH', 'VEHICLE', 'VTOL', 'NAVAL', 'SPHEROID_DROPSHIP',
      'AERO_FIGHTER', 'MECHWARRIOR', 'AERODYNE_DROPSHIP', 'BATTLESUIT' },
  },
  {
    page = 3, path = 'btech.unit.movement_types', cross = 'btech_types',
    names = { 'BIPED', 'TRACK', 'WHEEL', 'HOVER', 'VTOL', 'HULL', 'FOIL',
      'FLY', 'QUAD', 'SUB', 'NONE' },
  },
  {
    page = 3, path = 'btech.unit.sections', cross = 'btech_types',
    names = { 'FRONT_LEFT_LEG', 'FRONT_RIGHT_LEG', 'LEFT_TORSO',
      'RIGHT_TORSO', 'CENTER_TORSO', 'REAR_LEFT_LEG', 'REAR_RIGHT_LEG',
      'HEAD', 'LEFT_ARM', 'RIGHT_ARM', 'LEFT_LEG', 'RIGHT_LEG', 'SUIT_1',
      'SUIT_2', 'SUIT_3', 'SUIT_4', 'SUIT_5', 'SUIT_6', 'SUIT_7', 'SUIT_8',
      'LEFT_SIDE', 'RIGHT_SIDE', 'FRONT_SIDE', 'AFT_SIDE', 'TURRET', 'ROTOR',
      'NOSE', 'LEFT_WING', 'RIGHT_WING', 'LEFT_REAR_WING', 'RIGHT_REAR_WING',
      'AFT', 'FRONT_RIGHT_SIDE', 'FRONT_LEFT_SIDE', 'REAR_LEFT_SIDE',
      'REAR_RIGHT_SIDE' },
  },
  {
    page = 4, path = 'btech.unit.technology', cross = 'btech_types',
    names = { 'TRIPLE_STRENGTH_MYOMER', 'CLAN_ANTI_MISSILE',
      'INNER_SPHERE_ANTI_MISSILE', 'DOUBLE_HEAT_SINKS', 'MASC', 'CLAN',
      'FLIPPABLE_ARMS', 'C3_MASTER', 'C3_SLAVE', 'ARTEMIS_IV', 'ECM',
      'BEAGLE_PROBE', 'SALVAGE', 'CARGO', 'SEARCH_LIGHT', 'LIGHT_ACTIVE_PROBE',
      'ANTI_AIRCRAFT', 'NO_SENSORS', 'SIXTH_SENSE', 'FERRO_FIBROUS',
      'ENDO_STEEL', 'XL_ENGINE', 'ICE_ENGINE', 'SINGLE_HEAT_SINKS',
      'LIGHT_ENGINE', 'XXL_ENGINE', 'COMPACT_ENGINE', 'REINFORCED_INTERNAL',
      'COMPOSITE_INTERNAL', 'HARDENED_ARMOR', 'CRITICAL_PROOF',
      'STEALTH_ARMOR', 'HEAVY_FERRO_FIBROUS', 'LASER_REFLECTIVE_ARMOR',
      'REACTIVE_ARMOR', 'NULL_SIGNATURE_SYSTEM', 'C3I', 'SUPERCHARGER',
      'IMPROVED_JUMP_JETS', 'MECHANICAL_JUMP_JETS', 'COMPACT_HEAT_SINKS',
      'LASER_HEAT_SINKS', 'BLOODHOUND_PROBE', 'ANGEL_ECM', 'WATCHDOG',
      'LIGHT_FERRO_FIBROUS', 'TAG', 'OMNIMECH', 'ARTEMIS_V', 'CAMOUFLAGE',
      'CARRIER', 'WATERPROOF', 'XL_GYRO', 'HEAVY_DUTY_GYRO', 'COMPACT_GYRO',
      'TARGETING_COMPUTER', 'SMALL_COCKPIT', 'SWARM_ATTACK', 'MOUNT_FRIENDS',
      'ANTI_LEG_ATTACK', 'PURIFIER_STEALTH', 'KAGE_STEALTH', 'ACHILEUS_STEALTH',
      'INFILTRATOR_STEALTH', 'INFILTRATOR_II_STEALTH', 'MUST_JETTISON_PACK',
      'CAN_JETTISON_PACK' },
  },
  {
    page = 3, path = 'btech.unit.technology_groups', cross = 'btech_types',
    names = { 'UNIT', 'INFANTRY', 'ALL' },
  },
  {
    page = 5, path = 'btech.unit.fire_modes', cross = 'btech_types',
    names = { 'DESTROYED', 'DISABLED', 'BROKEN', 'DAMAGED',
      'TARGETING_COMPUTER', 'REAR_MOUNT', 'HOTLOAD', 'HALF_TON', 'ONE_SHOT',
      'ONE_SHOT_USED', 'ULTRA', 'RAPID_FIRE', 'GATLING', 'ROTARY_TWO_SHOT',
      'ROTARY_FOUR_SHOT', 'ROTARY_SIX_SHOT', 'HEAT', 'BACKPACK', 'JETTISONED',
      'OMNI_BASE', 'ROCKET_FIRED' },
  },
  {
    page = 5, path = 'btech.unit.ammunition_modes', cross = 'btech_types',
    names = { 'LBX_CLUSTER', 'ARTEMIS_MINE', 'NARC_SMOKE', 'CLUSTER', 'MINE',
      'SMOKE', 'INFERNO', 'SWARM', 'SWARM_1', 'INARC_EXPLOSIVE',
      'INARC_HAYWIRE', 'INARC_ECM', 'INARC_NEMESIS', 'ARMOR_PIERCING',
      'FLECHETTE', 'INCENDIARY', 'PRECISION', 'STINGER', 'CASELESS',
      'SEMI_GUIDED', 'EXTENDED_RANGE', 'HIGH_EXPLOSIVE', 'MML_LRM' },
  },
  {
    page = 5, path = 'btech.repair.operations', cross = 'btech_types',
    names = { 'REATTACH', 'REPAIR_PART', 'REPAIR_WEAPON_TEMPORARY',
      'REPAIR_ENHANCEMENT', 'REPAIR_FOCUS', 'REPAIR_CRYSTAL', 'REPAIR_BARREL',
      'REPAIR_AMMO_FEED', 'REPAIR_RANGING', 'REPAIR_AMMO_MOUNT',
      'REPLACE_WEAPON', 'RELOAD', 'REPAIR_ARMOR', 'REPAIR_REAR_ARMOR',
      'REPAIR_INTERNAL', 'DETACH', 'SCRAP_PART', 'SCRAP_WEAPON', 'UNLOAD',
      'RESEAL', 'REPLACE_SUIT' },
  },
  {
    page = 6, path = 'btech.autopilot.orders', cross = 'btech_types',
    names = { 'CHASE_TARGET', 'DUMB_FOLLOW', 'FOLLOW', 'EMBARK', 'PICK_UP',
      'DUMB_GOTO', 'GOTO', 'OLD_GOTO', 'ENTER_BASE', 'LEAVE_BASE', 'ROAM',
      'AUTO_GUN', 'DROP_OFF', 'SHUT_DOWN', 'START_UP', 'UNIT_DISEMBARK',
      'SPEED' },
  },
  {
    page = 6, path = 'btech.autopilot.directions', cross = 'btech_types',
    names = { 'NORTH', 'EAST', 'SOUTH', 'WEST' },
  },
  {
    page = 6, path = 'btech.autopilot.roam_modes', cross = 'btech_types',
    names = { 'MAP', 'RADIUS' },
  },
  {
    page = 6, path = 'btech.autopilot.autogun_modes', cross = 'btech_types',
    names = { 'AUTOMATIC', 'OFF', 'TARGET' },
  },
}

local function survey(spec)
  local namespace = resolve(spec.path)
  local cross = CROSS[spec.cross]
  local row = {
    name = spec.path,
    type = type(namespace),
    metatable = getmetatable(namespace),
    keys = {},
    constants = {},
  }
  local first
  for _, name in ipairs(spec.names) do
    row.keys[#row.keys + 1] = 'string:' .. name
    local ok, value = pcall(function() return namespace[name] end)
    if not ok then
      row.constants[name] = { lookup_error = descriptor(value) }
    else
      if first == nil then first = value end
      local record = descriptor(value)
      record.self = value == namespace[name]
      record.first = value == first
      record.cross = cross(value)
      row.constants[name] = record
    end
  end
  table.sort(row.keys)
  row.errors = {
    unknown = edge(function() return namespace.NO_SUCH end),
    numeric = edge(function() return namespace[42] end),
    false_key = edge(function() return namespace[false] end),
    mutate = edge(function() namespace.NEW = 1 end),
    mutate_constant = edge(function() namespace[spec.names[1]].NEW = 1 end),
  }
  return row
end

local function page(number)
  local namespaces = {}
  for _, spec in ipairs(SPECS) do
    if spec.page == number then namespaces[#namespaces + 1] = survey(spec) end
  end
  local record = { page = number, namespaces = namespaces }
  if number == 1 then
    record.__next_commands = { 'luaparity2', 'luaparity3', 'luaparity4',
      'luaparity5', 'luaparity6' }
  end
  return json(record)
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
      emit(ctx.enactor, page(1))
      return true
    end,
  }}
for number = 2, 6 do
  local page_number = number
  commands[#commands + 1] = {
    name = 'lua-parity-probe-' .. page_number, permission = 'everyone',
    pattern = '^luaparity' .. page_number .. '$',
    handler = function(ctx) emit(ctx.enactor, page(page_number)); return true end,
  }
end
return { commands = commands }
