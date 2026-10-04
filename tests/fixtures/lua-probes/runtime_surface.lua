-- This probe exercises invalid calls and native internals on purpose.
---@diagnostic disable: undefined-field, undefined-global
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

local function run()
  local globals = {}
  for name in pairs(_G) do globals[#globals + 1] = tostring(name) end
  table.sort(globals)
  local records = {
    globals = globals,
    namespaces = {
      mux = namespace('mux', mux), mux_error = namespace('mux.error', mux.error),
      mux_error_codes = namespace('mux.error.codes', mux.error.codes),
      mux_world = namespace('mux.world', mux.world), btech = namespace('btech', btech),
      btech_error = namespace('btech.error', btech.error),
      btech_error_codes = namespace('btech.error.codes', btech.error.codes),
      btech_unit = namespace('btech.unit', btech.unit),
    },
    values = {
      select_missing = returns(select(2, 'only')),
      select_values = returns(nil, false, 'x'),
      return_serializer_check = returns({a = 1}, 'tail', nil),
      select_count = descriptor(select('#', nil, false, 'x')),
      math_pi = descriptor(math.pi), math_huge = descriptor(math.huge),
      table_move = descriptor(table.move), gcinfo = descriptor(gcinfo),
      newproxy = descriptor(newproxy), math_mod = descriptor(math.mod),
      unit_type = descriptor(btech.unit.types.MECH),
      unit_type_equal = descriptor(btech.unit.types.MECH == btech.unit.types.MECH),
      mux_arg_code = descriptor(mux.error.codes.arg.invalid),
    },
  }
  return json(records)
end

return {
  commands = {{
    name = 'lua-parity-probe',
    permission = 'everyone',
    pattern = '^luaparity$',
    handler = function(ctx)
      mux.world.pemit(ctx.enactor, 'LUA_PARITY:' .. run())
      return true
    end,
  }},
}
