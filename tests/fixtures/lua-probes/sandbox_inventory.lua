-- Differential probe: sandbox library inventory and removed runtime-control
-- globals. C lua_install_sandbox (lua_runtime.c:270-313) nils sixteen
-- runtime-control globals and reinstalls only the module loader, so the
-- coroutine/GC/environment entry points stay absent while string, table, math
-- and bit remain installed.

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
    result.bytes = value:gsub('.', function(byte) return string.format('%02x', string.byte(byte)) end)
  elseif kind == 'function' or kind == 'userdata' then
    result.text = tostring(value)
  end
  return result
end

local function returns(...)
  local result = pack(...)
  local row = { count = result.n }
  for index = 1, result.n do row[index] = descriptor(result[index]) end
  return row
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

local function sorted_keys(value)
  local keys = {}
  for key in pairs(value) do keys[#keys + 1] = tostring(key) end
  table.sort(keys)
  return keys
end

local function run()
  local globals = sorted_keys(_G)
  return json({
    globals = globals,
    libraries = {
      string = sorted_keys(string),
      table = sorted_keys(table),
      math = sorted_keys(math),
      bit = sorted_keys(bit),
    },
    removed = {
      collectgarbage = descriptor(collectgarbage),
      coroutine = descriptor(coroutine),
      getfenv = descriptor(getfenv),
      load = descriptor(load),
      loadstring = descriptor(loadstring),
      module = descriptor(module),
      package = descriptor(package),
      setfenv = descriptor(setfenv),
      io = descriptor(io),
      os = descriptor(os),
      debug = descriptor(debug),
      dofile = descriptor(dofile),
      loadfile = descriptor(loadfile),
    },
    removed_index = {
      io = returns(_G[string.rep('i', 1) .. string.rep('o', 1)]),
      package_prefixed = returns(_G['package.loaded']),
      coroutine_nested = returns(_G['coroutine.create']),
      rawget_load = returns(rawget(_G, string.rep('loa', 1) .. 'd')),
    },
    -- The library tables stay installed and their core routines callable.
    adjacent = {
      string_rep = returns(string.rep('ab', 2)),
      string_format = returns(string.format('%d-%s', 7, 'x')),
      string_byte = returns(string.byte('A')),
      table_concat = returns(table.concat({ 'a', 'b' }, '+')),
      table_insert = (function()
        local list = { 1 }
        table.insert(list, 2)
        return returns(list[1], list[2], #list)
      end)(),
      math_abs = returns(math.abs(-3)),
      math_max = returns(math.max(2, 5)),
      bit_band = returns(bit.band(12, 10)),
      bit_tobit = returns(bit.tobit(4294967290)),
      bit_tohex = returns(bit.tohex(255)),
      pcall_error = returns(pcall(error, 'boom')),
      xpcall_ok = returns(xpcall(function() return 1 end, function() return 'h' end)),
    },
    require_semantics = {
      -- C lua_require_module special-cases only 'btech'; there is no mux.lua
      -- module, so require('mux') is deliberately not probed here.
      btech_identity = returns(rawequal(require('btech'), btech)),
      missing = capture(require, 'no_such_module'),
      invalid_name = capture(require, '.leading'),
      string_library = capture(require, 'string'),
      number_argument = capture(require, 42),
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

local commands = {{
  name = 'lua-parity-probe', permission = 'everyone', pattern = '^luaparity$',
  handler = function(ctx)
    emit(ctx.enactor, run())
    return true
  end,
}}
return { commands = commands }
