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
    result.value = string.format('%.17g', value)
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

local function run()
  local store = mux.world.object(1)
  local part = assert(btech.parts.resolve('Agra.IS.PPC'))
  local sibling
  for _, candidate in ipairs(btech.parts.list('weapon')) do
    if candidate.id == part.id and candidate.brand ~= part.brand then sibling = candidate; break end
  end
  assert(sibling)
  local result = {
    initial = btech.parts.store_quantity(store, part),
    add = capture(btech.parts.adjust_stores, store, part, 3),
  }
  result.after_add = btech.parts.store_quantity(store, part)
  local stores = btech.parts.stores(store)
  result.stores_after_add = descriptor(stores)
  result.remove = capture(btech.parts.adjust_stores, store, part, -3)
  result.after_remove = btech.parts.store_quantity(store, part)
  result.negative = capture(btech.parts.adjust_stores, store, part, -1)
  result.stores_after_zero = #btech.parts.stores(store)
  result.adjust_inputs = {
    omitted = capture(btech.parts.adjust_stores, store, part),
    nil_value = capture(btech.parts.adjust_stores, store, part, nil),
    false_value = capture(btech.parts.adjust_stores, store, part, false),
    numeric_string = capture(btech.parts.adjust_stores, store, part, '3'),
    zero = capture(btech.parts.adjust_stores, store, part, 0),
    fractional = capture(btech.parts.adjust_stores, store, part, 1.5),
  }
  result.capacity = {
    fill = capture(btech.parts.adjust_stores, store, part, 2147483647),
    quantity = btech.parts.store_quantity(store, part),
    overflow = capture(btech.parts.adjust_stores, store, part, 1),
    clear = capture(btech.parts.adjust_stores, store, part, -2147483647),
  }
  result.set_cost = capture(btech.parts.set_cost, part, 123456)
  result.same_cost = btech.parts.resolve(part).cost
  result.sibling_cost = btech.parts.resolve(sibling).cost
  result.zero_cost = capture(btech.parts.set_cost, sibling, 0)
  result.final_cost = btech.parts.resolve(part).cost
  result.cost_inputs = {
    omitted = capture(btech.parts.set_cost, part),
    nil_value = capture(btech.parts.set_cost, part, nil),
    false_value = capture(btech.parts.set_cost, part, false),
    numeric_string = capture(btech.parts.set_cost, part, '123'),
    negative = capture(btech.parts.set_cost, part, -1),
    fractional = capture(btech.parts.set_cost, part, 1.5),
    maximum = capture(btech.parts.set_cost, part, 9007199254740991),
    reset = capture(btech.parts.set_cost, part, 0),
  }
  return json(result)
end

local function emit(target, payload)
  local size = 3000
  local count = math.max(1, math.ceil(#payload / size))
  for index = 1, count do
    mux.world.pemit(target, 'LUA_PARITY:' .. index .. '/' .. count .. ':' ..
      payload:sub(((index - 1) * size) + 1, index * size))
  end
end

return {
  commands = {{
    name = 'lua-parity-probe', permission = 'everyone', pattern = '^luaparity$',
    handler = function(ctx)
      emit(ctx.enactor, run())
      return true
    end,
  }},
}
