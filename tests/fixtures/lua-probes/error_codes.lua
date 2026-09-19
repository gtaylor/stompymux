-- Differential probe: mux.error/btech.error code trees, plain-table nodes and raises.
-- Oracle: btmux-khi/src/mux/lua/lua_error.c and .../mux/error/mux_error_bindings.c.
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

-- Collect every leaf code of a tree in dotted order; nodes are plain tables.
local function leaves(node, prefix, out)
  local has_child = false
  for key, value in pairs(node) do
    if key ~= 'code' and type(value) == 'table' then
      has_child = true
      leaves(value, prefix .. key .. '.', out)
    end
  end
  if not has_child then
    out[#out + 1] = prefix:gsub('%.$', '')
  end
  table.sort(out)
  return out
end

local function capture(call)
  local function pack(...) return { n = select('#', ...), ... } end
  local packed = pack(pcall(call))
  local failure = packed[2]
  if type(failure) == 'table' then
    failure = { code = failure.code, message = failure.message, detail = failure.detail }
  end
  return { count = packed.n, ok = descriptor(packed[1]), error = descriptor(failure) }
end

local function run()
  local mux_codes = mux.error.code_tree('mux')
  local btech_codes = mux.error.code_tree('btech')
  local testing_codes = mux.error.code_tree('testing')
  local mux_leaves = leaves(mux_codes, '', {})
  local btech_leaves = leaves(btech_codes, '', {})
  local testing_leaves = leaves(testing_codes, '', {})
  local node_rows = {}
  local raise_rows = {}
  for _, code in ipairs(mux_leaves) do
    local node = mux_codes
    for segment in code:gmatch('[^%.]+') do node = node[segment] end
    node_rows[#node_rows + 1] = {
      code = code,
      node_type = type(node),
      field = descriptor(node.code),
      text = descriptor(tostring(node)),
      table_type = type(mux.error.codes) ,
      self_equal = descriptor(node == node),
    }
    raise_rows[#raise_rows + 1] = capture(function()
      return mux.error.raise(node, 'parity raise')
    end)
  end
  for _, code in ipairs(btech_leaves) do
    local node = btech_codes
    for segment in code:gmatch('[^%.]+') do node = node[segment] end
    node_rows[#node_rows + 1] = {
      code = code,
      node_type = type(node),
      field = descriptor(node.code),
      text = descriptor(tostring(node)),
      table_type = 'skip',
      self_equal = descriptor(node == node),
    }
    raise_rows[#raise_rows + 1] = capture(function()
      return mux.error.raise(node, 'parity raise')
    end)
  end
  for _, code in ipairs(testing_leaves) do
    local node = testing_codes
    for segment in code:gmatch('[^%.]+') do node = node[segment] end
    node_rows[#node_rows + 1] = {
      code = code,
      node_type = type(node),
      field = descriptor(node.code),
      text = descriptor(tostring(node)),
      table_type = 'skip',
      self_equal = descriptor(node == node),
    }
  end
  return json({
    mux_leaf_codes = mux_leaves,
    btech_leaf_codes = btech_leaves,
    testing_leaf_codes = testing_leaves,
    nodes = node_rows,
    raises = raise_rows,
    identity = {
      codes_equal_tree = descriptor(mux.error.codes == mux_codes),
      tree_cached = descriptor(mux.error.code_tree('mux') == mux_codes),
      btech_cached = descriptor(mux.error.code_tree('btech') == btech_codes),
      cross_inequality = descriptor(
        mux_codes.arg.invalid ~= btech_codes.part.not_found
          and mux_codes.arg.invalid ~= testing_codes.assertion
      ),
    },
    branches = {
      arg_text = descriptor(tostring(mux_codes.arg)),
      unavailable_text = descriptor(tostring(mux_codes.unavailable)),
      root_text = descriptor(tostring(mux_codes)),
      btech_root_text = descriptor(tostring(btech_codes)),
      state_branch_codes = descriptor({
        tostring(mux_codes.state.invalid), tostring(mux_codes.state.value_too_large),
        tostring(mux_codes.state.unavailable),
      }),
    },
    existing_field_writability = capture(function()
      local node = mux_codes.arg.invalid
      local original = node.code
      node.code = 'authored'
      local changed = tostring(node)
      node.code = original
      return changed
    end),
    mutation = {
      new_field = capture(function() mux_codes.arg.forged = true end),
      new_root_field = capture(function() mux_codes.forged = true end),
      new_btech_field = capture(function() btech_codes.part.forged = true end),
    },
    lookups = {
      unknown_segment = capture(function() return mux_codes.arg.no_such end),
      unknown_root_segment = capture(function() return mux_codes.no_such end),
      non_string_key = capture(function() return mux_codes[{}] end),
      number_key = capture(function() return mux_codes[42] end),
      unknown_root = capture(function() return mux.error.code_tree('author') end),
      number_root = capture(function() return mux.error.code_tree(42) end),
      non_string_root = capture(function() return mux.error.code_tree(false) end),
    },
    code_tree_errors_are_structured = capture(function()
      return mux.error.code_tree('author')
    end),
    is_matching = {
      exact = descriptor(mux.error.is({ code = 'mux.arg.invalid' }, mux_codes.arg.invalid)),
      prefix = descriptor(mux.error.is({ code = 'mux.arg.invalid.child' }, 'mux.arg')),
      cross_root = descriptor(mux.error.is({ code = 'btech.part.not_found' }, 'mux')),
      raw_code = descriptor(mux.error.is({ code = 'mux.runtime' }, 'mux.runtime')),
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

return {commands={{
  name='lua-parity-probe', permission='everyone', pattern='^luaparity$',
  handler=function(ctx)
    emit(ctx.enactor, run())
    return true
  end,
}}}
