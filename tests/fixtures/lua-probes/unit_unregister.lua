-- Differential probe: @btech/unregister teardown for MECH-registered units.
--
-- The native @btech command is the only registrar in either binary, so the
-- register/inspect/unregister dialogue runs as PARITY_SETUP socket lines. The
-- first @pemit below emits the output-capture bracket straight to the socket:
-- every later setup reply is diffed byte-for-byte through captured_output_bytes,
-- covering exact success strings, dbrefs, double unregister (the reference
-- succeeds for an already-plain object), and post-teardown inspect output.
-- PARITY_SETUP: @pemit me=LUA_PARITY_OUTPUT_BEGIN
-- PARITY_SETUP: luaparity0
-- PARITY_SETUP: @btech/register Parity Unregister Bare=MECH
-- PARITY_SETUP: @btech/register Parity Unregister Loaded=MECH
-- PARITY_SETUP: @btech/register Parity Unregister Control=MECH
-- PARITY_SETUP: @btech/info Parity Unregister Bare
-- PARITY_SETUP: luaparity9
-- PARITY_SETUP: @btech/unregister Parity Unregister Bare
-- PARITY_SETUP: @btech/unregister Parity Unregister Bare
-- PARITY_SETUP: @btech/info Parity Unregister Bare
-- PARITY_SETUP: @btech/unregister Parity Unregister Loaded
-- PARITY_SETUP: @btech/info Parity Unregister Loaded
-- PARITY_SETUP: @btech/info Parity Unregister Control

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

local function unit_api() return btech.unit end

local function page_post_teardown()
  local unit = unit_api()
  return {
    -- Both unregistered objects behave like plain things: every unit entry point
    -- fails the registration check with the shared structured error.
    bare_load = capture(unit.load_template, state.bare, 'PARITY-PROBE'),
    bare_scalar = capture(unit.set_max_speed, state.bare, 5),
    loaded_identity = capture(unit.preferred_id, state.loaded),
    loaded_handle = capture(unit.assigned_pilot, state.loaded),
    bare_handle = capture(unit.display_name, state.bare),
    -- The never-unregistered control unit keeps its MECH registration and still
    -- constructs; the zone scan sees exactly that one registered thing.
    zone_units = capture(btech.system.units_in_zone, mux.world.object(0)),
    control_load = capture(unit.load_template, state.control, 'PARITY-PROBE'),
    zone_units_after_control_load = capture(btech.system.units_in_zone, mux.world.object(0)),
    __capture_output = true,
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
  page_post_teardown,
}

local commands = {{
  name = 'lua-parity-probe-prepare', permission = 'everyone', pattern = '^luaparity0$',
  handler = function(ctx)
    local function create(name)
      return mux.world.create_object({
        type = mux.world.types.THING, name = name, location = mux.world.object(0),
      })
    end
    state.bare = create('Parity Unregister Bare')
    state.loaded = create('Parity Unregister Loaded')
    state.control = create('Parity Unregister Control')
    return true
  end,
}, {
  -- Silent loader: construct the template-loaded unit before its teardown so the
  -- captured unregister covers the live-constructed path, not only raw roles.
  name = 'lua-parity-probe-load', permission = 'everyone', pattern = '^luaparity9$',
  handler = function(ctx)
    btech.unit.load_template(state.loaded, 'PARITY-PROBE')
    btech.unit.set_display_name(state.loaded, 'Loaded Probe')
    btech.unit.set_assigned_pilot(state.loaded, mux.world.object(1))
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
