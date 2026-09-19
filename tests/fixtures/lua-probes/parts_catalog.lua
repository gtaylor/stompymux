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

local function first_and_count(rows)
  return { count = #rows, first = descriptor(rows[1]), last = descriptor(rows[#rows]) }
end

local function identities(rows)
  local result = {}
  for _, row in ipairs(rows) do
    result[#result + 1] = row.id .. ':' .. row.brand .. ':' .. row.very_long_name
  end
  return result
end

local function capture(call)
  return returns(pcall(call))
end

local function run()
  local all_parts = btech.parts.list()
  local category_counts = {}
  for _, code in ipairs({'weapon', 'ammunition', 'bomb', 'special', 'cargo', 'other'}) do
    category_counts[code] = #btech.parts.list(code)
  end
  return json({
    categories = returns(btech.parts.categories()),
    all = first_and_count(all_parts),
    all_records = descriptor({unpack(all_parts, 1, math.min(100, #all_parts))}),
    __next_commands = {'luaparity2', 'luaparity3', 'luaparity4', 'luaparity5'},
    identities = identities(all_parts),
    category_counts = category_counts,
    exact_search = first_and_count(btech.parts.search('Agra.IS.PPC')),
    wildcard_search = first_and_count(btech.parts.search('*PPC*')),
    resolve = {
      bare = capture(function() return btech.parts.resolve('IS.PPC') end),
      branded = capture(function() return btech.parts.resolve('Agra.IS.PPC') end),
      packed_brand_five = capture(function() return btech.parts.resolve(5197) end),
      record_brand_five = capture(function() return btech.parts.resolve({ id = 77, brand = 5 }) end),
      record_brand_zero = capture(function() return btech.parts.resolve({ id = 77, brand = 0 }) end),
      numeric_zero = capture(function() return btech.parts.resolve(77) end),
      numeric_string = capture(function() return btech.parts.resolve('5197') end),
      missing = capture(function() return btech.parts.resolve('does-not-exist') end),
      nil_value = capture(function() return btech.parts.resolve(nil) end),
      false_value = capture(function() return btech.parts.resolve(false) end),
      fractional = capture(function() return btech.parts.resolve(1.5) end),
      bad_record = capture(function() return btech.parts.resolve({ id = 77 }) end),
      extra_args = capture(function() return btech.parts.resolve('Agra.IS.PPC', 'ignored') end),
    },
    list_inputs = {
      omitted = returns(true, #all_parts),
      explicit_nil = capture(function() return #btech.parts.list(nil) end),
      uppercase = capture(function() return #btech.parts.list('WEAPON') end),
      unknown = capture(function() return btech.parts.list('unknown') end),
      false_value = capture(function() return btech.parts.list(false) end),
      extra_args = capture(function() return #btech.parts.list(nil, 'ignored') end),
    },
    search_inputs = {
      omitted = capture(function() return btech.parts.search() end),
      empty = capture(function() return btech.parts.search('') end),
      false_value = capture(function() return btech.parts.search(false) end),
      exact = capture(function() return #btech.parts.search('Agra.IS.PPC') end),
      extra_args = capture(function() return #btech.parts.search('*PPC*', 'ignored') end),
    },
  })
end

local function page(number)
  local rows = btech.parts.list()
  local first = ((number - 1) * 100) + 1
  local last = math.min(number * 100, #rows)
  return json({page = number, records = descriptor({unpack(rows, first, last)})})
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
for number = 2, 5 do
  local page_number = number
  commands[#commands + 1] = {
    name = 'lua-parity-probe-' .. page_number, permission = 'everyone',
    pattern = '^luaparity' .. page_number .. '$',
    handler = function(ctx) emit(ctx.enactor, page(page_number)); return true end,
  }
end
return {commands = commands}
