-- This probe exercises invalid calls and native internals on purpose.
---@diagnostic disable: inject-field
-- Differential probe: canonical btech package identity, subpackage stability,
-- typed-catalog immutability, and constant __eq semantics (typed vs untyped).
-- Oracle: btmux-khi/src/mux/lua/packages/btech/btech_package.c install order,
-- btech_constants.c, lua_runtime.c lua_require_module, and the mux __eq
-- handlers in mux_flag_power_bindings.c, mux_comsys_*_bindings.c,
-- mux_lock_bindings.c, mux_object_type_bindings.c, command_access.c.
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

-- Plain Lua errors raised from C luaL_checkudata __eq handlers carry the
-- caller's incidental source spelling ("<path>:<line>: bad argument ...").
-- Keep the argument position, function name and detail; drop only the prefix.
local function plain_message(message)
  if type(message) ~= 'string' then return message end
  return message:match("(bad argument .*") or message
end

local function capture(call)
  local function pack(...) return { n = select('#', ...), ... } end
  local packed = pack(pcall(call))
  local failure = packed[2]
  if type(failure) == 'table' then
    failure = { code = failure.code, message = failure.message, detail = failure.detail }
  elseif type(failure) == 'string' then
    failure = plain_message(failure)
  end
  return { count = packed.n, ok = descriptor(packed[1]), error = descriptor(failure) }
end

local function run()
  local required = require('btech')
  local subpackages = {}
  for _, name in ipairs({
    'autopilot', 'character', 'error', 'map', 'parts', 'player',
    'repair', 'system', 'template', 'unit',
  }) do
    subpackages[#subpackages + 1] = {
      name = name,
      same_table = btech[name] == required[name],
      stable_across_require = required[name] == require('btech')[name],
      value_type = type(btech[name]),
    }
  end
  local channel = mux.comsys.create_channel('eqprobe')
  local eq = {
    -- Both engines are LuaJIT: __eq is dispatched only when both operands
    -- share one metatable, so every cross-family comparison below is plainly
    -- false without calling the metamethods (lua_mux_constant_equal's untyped
    -- raw {package, id} read and the typed families' luaL_checkudata errors
    -- never fire across metatables). Same-family pairs below exercise the
    -- handlers: values, identities and generations compare as documented.
    flags_ansi_eq_powers_idle = descriptor(mux.world.flags.ANSI == mux.world.powers.IDLE),
    powers_idle_eq_flags_ansi = descriptor(mux.world.powers.IDLE == mux.world.flags.ANSI),
    flags_wizard_eq_powers_idle = descriptor(mux.world.flags.WIZARD == mux.world.powers.IDLE),
    flags_ansi_eq_types_thing = descriptor(mux.world.flags.ANSI == mux.world.types.THING),
    flags_ansi_eq_types_room = descriptor(mux.world.flags.ANSI == mux.world.types.ROOM),
    flags_ansi_eq_comsys_public = descriptor(mux.world.flags.ANSI == mux.comsys.flags.PUBLIC),
    powers_idle_eq_types_thing = descriptor(mux.world.powers.IDLE == mux.world.types.THING),
    comsys_flag_eq_world_flag = capture(function() return mux.comsys.flags.PUBLIC == mux.world.flags.ANSI end),
    types_thing_eq_world_flag = capture(function() return mux.world.types.THING == mux.world.flags.ANSI end),
    lock_eq_comsys_flag = capture(function() return mux.world.locks.TAKE == mux.comsys.flags.PUBLIC end),
    comsys_flag_eq_lock = capture(function() return mux.comsys.flags.PUBLIC == mux.world.locks.TAKE end),
    type_eq_access = capture(function() return mux.world.types.ROOM == mux.world.access.WIZARD end),
    access_eq_type = capture(function() return mux.world.access.WIZARD == mux.world.types.ROOM end),
    btech_mech_eq_comsys_flag = capture(function() return btech.unit.types.MECH == mux.comsys.flags.PUBLIC end),
    comsys_flag_eq_btech_mech = capture(function() return mux.comsys.flags.PUBLIC == btech.unit.types.MECH end),
    channel_eq_world_flag = capture(function() return channel == mux.world.flags.ANSI end),
    object_eq_world_flag = capture(function() return mux.world.object(1) == mux.world.flags.ANSI end),
    -- Same-family equality keeps comparing values, packages and identities.
    comsys_public_eq_public = descriptor(mux.comsys.flags.PUBLIC == mux.comsys.flags.PUBLIC),
    comsys_public_ne_loud = descriptor(mux.comsys.flags.PUBLIC ~= mux.comsys.flags.LOUD),
    lock_eq_lock = descriptor(mux.world.locks.TAKE == mux.world.locks.TAKE),
    type_eq_type = descriptor(mux.world.types.ROOM == mux.world.types.ROOM),
    access_eq_access = descriptor(mux.world.access.WIZARD == mux.world.access.WIZARD),
    btech_mech_eq_mech = descriptor(btech.unit.types.MECH == btech.unit.types.MECH),
    btech_cross_catalog = descriptor(btech.unit.types.MECH == btech.unit.movement_types.TRACK),
    btech_same_value_cross_catalog = descriptor(btech.unit.fire_modes.DESTROYED == btech.unit.types.VEHICLE),
    channel_eq_channel = descriptor(channel == mux.comsys.channel('EQPROBE')),
    object_eq_object = descriptor(mux.world.object(1) == mux.world.object(1)),
  }
  mux.comsys.destroy_channel(channel)
  return json({
    identity = {
      require_is_global = descriptor(required == btech),
      require_cached = descriptor(require('btech') == required),
      require_type = type(required),
      global_type = type(btech),
      subpackages = subpackages,
      unit_namespace_stable = descriptor(
        btech.unit.types == required.unit.types
          and btech.autopilot.orders == required.autopilot.orders
          and btech.repair.operations == required.repair.operations),
      error_codes_equal = descriptor(btech.error.codes == btech.error.codes),
      error_codes_type = type(btech.error.codes),
    },
    constants = {
      mech_rawequal = descriptor(rawequal(btech.unit.types.MECH, btech.unit.types.MECH)),
      namespace_rawequal = descriptor(rawequal(btech.unit.types, btech.unit.types)),
      namespace_metatable = descriptor(getmetatable(btech.unit.types)),
      constant_metatable = descriptor(getmetatable(btech.unit.types.MECH)),
      autopilot_metatable = descriptor(getmetatable(btech.autopilot.orders)),
      namespace_new = capture(function() btech.unit.types.FORGED = 1 end),
      constant_assign = capture(function() btech.unit.types.MECH = 1 end),
      orders_new = capture(function() btech.autopilot.orders.FORGED = 1 end),
      operations_new = capture(function() btech.repair.operations.FORGED = 1 end),
      codes_new = capture(function() btech.error.codes.operation.FORGED = 1 end),
    },
    eq = eq,
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
