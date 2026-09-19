-- Differential probe: shipped Lua packages (testing, access_policy,
-- object_appearances) loaded through require from the seeded game directory.
--
-- Both isolated games run the byte-identical pinned module sources, so this
-- probe audits module behavior over fixed inputs rather than file bytes.
-- Captured rows avoid raw string errors whose raise positions embed the
-- per-run game directory; the Rust-side suite covers those spellings.

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
    -- Plain assert() failures inside the shipped modules carry their raise
    -- position, whose chunk spelling embeds the per-run game directory.
    -- Normalize the prefix so both sides compare the shared module line and
    -- message text.
    local message = result[2]
    if type(message) == 'string' then
      local line, tail = message:match('^[^:]+:(%d+): (.*)$')
      if line then message = '<module>:' .. line .. ': ' .. tail end
    end
    row.error = descriptor(message)
  end
  return row
end

local function returns(...)
  local result = pack(...)
  local row = { count = result.n }
  for index = 1, result.n do row[index] = descriptor(result[index]) end
  return row
end

local function testing_rows()
  local testing = require('testing')
  local keys = {}
  for key in pairs(testing) do keys[#keys + 1] = key end
  table.sort(keys)
  local codes = testing.error.codes
  local suite = testing.suite('parity', { tests = {} })
  local expect = suite.expect
  local other = testing.suite('other', { tests = {} })
  local case = testing.test('case', function() end)
  local structured = mux.error.new { code = 'mux.runtime', message = 'structured' }
  return {
    keys = keys,
    cache_identity = returns(rawequal(testing, require('testing'))),
    separate_modules = returns(rawequal(testing, require('access_policy'))),
    codes_identity = returns(rawequal(codes, mux.error.code_tree('testing'))),
    assertion_code = returns(tostring(codes.assertion), codes.assertion.code),
    runtime_code = returns(tostring(codes.runtime), codes.runtime.code),
    code_mutation = capture(function() codes.assertion.extra = true end),
    code_unknown = capture(function() return codes.unknown end),
    case_shape = { name = case.name, run_type = type(case.run) },
    suite_shape = { name = suite.name, tests_type = type(suite.tests),
      expect_identity = rawequal(expect, other.expect) },
    suite_bad_name = capture(function() return testing.suite('', { tests = {} }) end),
    suite_bad_definition = capture(function() return testing.suite('x', 7) end),
    suite_bad_tests = capture(function() return testing.suite('x', { tests = 7 }) end),
    test_bad_name = capture(function() return testing.test('', function() end) end),
    test_bad_callback = capture(function() return testing.test('x', 7) end),
    expect_pass = {
      equal = capture(expect.equal, 1, 1),
      not_equal = capture(expect.not_equal, 1, 2),
      truthy = capture(expect.truthy, 'yes'),
      falsy = capture(expect.falsy, nil),
      is_nil = capture(expect.is_nil, nil),
      contains_string = capture(expect.contains, 'hello world', 'world'),
      contains_table = capture(expect.contains, { 10, 20, 30 }, 20),
      near_default = capture(expect.near, 0.5, 0.5),
      near_tolerance = capture(expect.near, 1, 1.5, 0.6),
      error_matches = capture(expect.error_matches,
        function() error(structured) end, 'structured'),
      raises = capture(expect.raises, function() error(structured) end),
      raises_code = capture(expect.raises_code,
        function() error(structured) end, 'mux.runtime'),
      no_error = capture(expect.no_error, function() return 7 end),
      is_error = capture(expect.is_error, structured, 'mux.runtime'),
    },
    expect_fail = {
      equal = capture(expect.equal, 1, 2),
      not_equal = capture(expect.not_equal, 1, 1),
      truthy = capture(expect.truthy, false),
      falsy = capture(expect.falsy, 'x'),
      is_nil = capture(expect.is_nil, 0),
      contains_string = capture(expect.contains, 'abc', 'z'),
      near_string = capture(expect.near, 'a', 1),
      near_out_of_range = capture(expect.near, 2, 1),
      error_matches_no_raise = capture(expect.error_matches, function() end, 'x'),
      error_matches_mismatch = capture(expect.error_matches,
        function() error(structured) end, 'silent'),
      raises_no_raise = capture(expect.raises, function() end),
      raises_code_mismatch = capture(expect.raises_code,
        function() error(structured) end, 'mux.arg.invalid'),
      no_error_raise = capture(expect.no_error, function() error(structured) end),
      is_error_mismatch = capture(expect.is_error, structured, 'mux.arg.invalid'),
    },
  }
end

local function access_policy_rows()
  local policy = require('access_policy')
  local keys = {}
  for key in pairs(policy) do keys[#keys + 1] = key end
  table.sort(keys)
  local god = mux.world.object(1)
  local home = mux.world.object(0)
  local holder = mux.world.create_object { type = mux.world.types.THING,
    name = 'Parity Policy Holder', location = home }
  local subject = mux.world.create_object { type = mux.world.types.THING,
    name = 'Parity Policy Subject', location = home }
  subject:set_affiliation(god)
  subject:state('identity'):set('rank', 'officer')
  subject:state('identity'):set('clearance', 7)
  local function evaluate(options)
    return policy.evaluate({ object = holder:dbref(), subject = subject:dbref() }, options)
  end
  local function entry(key, value) holder:state('lock'):set(key, value) end
  entry('flag/WIZARD', true)
  entry('affiliation', god:dbref())
  entry('state/identity/rank', 'officer')
  entry('state/identity/clearance', 7)
  entry('message/enactor', 'denied')
  entry('message/others', 'blocked')
  local rows = {
    keys = keys,
    empty_passes = capture(evaluate, { namespace = 'empty' }),
    full_passes = returns(evaluate({ namespace = 'lock' })),
    flag_mismatch = (function()
      subject:set_affiliation(nil)
      return nil
    end)(),
  }
  -- Flag requirement fails: the subject thing carries no WIZARD flag.
  rows.flag_denial = descriptor(evaluate({ namespace = 'lock' }))
  entry('flag/WIZARD', false)
  rows.flag_pass_after_relax = returns(evaluate({ namespace = 'lock' }))
  entry('flag/WIZARD', nil)
  entry('affiliation', nil)
  -- State mismatches deny with option-supplied default messages.
  entry('state/identity/clearance', 8)
  rows.state_denial = descriptor(evaluate({ namespace = 'lock',
    enactor_message = 'default no', other_message = 'default blocked' }))
  entry('state/identity/clearance', '7')
  rows.state_type_denial = descriptor(evaluate({ namespace = 'lock' }))
  holder:state('lock'):set('state/identity/clearance', nil)
  holder:state('lock'):set('state/identity/rank', nil)
  -- Malformed entries raise policy errors and therefore fail closed.
  local function policy_error(key, value, probe)
    entry(key, value)
    local row = capture(evaluate, { namespace = 'lock' })
    holder:state('lock'):set(key, nil)
    rows[probe] = row
  end
  policy_error('bogus', 'x', 'error_unknown_key')
  policy_error('flag/WIZARD', 1, 'error_flag_boolean')
  policy_error('flag/NOT_A_FLAG', true, 'error_flag_unknown')
  policy_error('affiliation', -3, 'error_affiliation_negative')
  policy_error('affiliation', 99999, 'error_affiliation_dead')
  policy_error('affiliation', 7.5, 'error_affiliation_fraction')
  policy_error('message/enactor', 5, 'error_message_enactor')
  policy_error('message/others', false, 'error_message_others')
  policy_error('state/nope/key', 1, 'error_state_shape')
  rows.context_bad = capture(function()
    return policy.evaluate(false, { namespace = 'lock' })
  end)
  rows.options_bad = capture(function()
    return policy.evaluate({ object = holder:dbref() }, 7)
  end)
  rows.namespace_missing = capture(function()
    return policy.evaluate({ object = holder:dbref(), subject = subject:dbref() }, {})
  end)
  return rows
end

local function appearance_rows()
  local appearance = require('object_appearances')
  local keys = {}
  for key in pairs(appearance) do keys[#keys + 1] = key end
  table.sort(keys)
  local god = mux.world.object(1)
  local room = mux.world.create_object { type = mux.world.types.ROOM,
    name = 'Parity Appearance Room', zone = 0 }
  mux.world.create_object { type = mux.world.types.THING, name = 'Parity Crate',
    location = room }
  mux.world.create_object { type = mux.world.types.THING,
    name = 'A Very Long Parity Thing Name That Exceeds The Column Width',
    location = room }
  mux.world.create_object { type = mux.world.types.EXIT, name = 'north;n;depart',
    location = room, destination = mux.world.object(0) }
  mux.world.create_object { type = mux.world.types.EXIT, name = 'south',
    location = room, destination = mux.world.object(0) }
  local ctx = { object = room:dbref(), enactor = god:dbref() }
  return {
    keys = keys,
    contents = descriptor(appearance.render_contents(ctx)),
    exits = descriptor(appearance.render_exits(ctx)),
    internal = descriptor(appearance.render_internal_appearance(ctx)),
    empty_room = (function()
      local bare = mux.world.create_object { type = mux.world.types.ROOM,
        name = 'Bare Room', zone = 0 }
      local bare_ctx = { object = bare:dbref(), enactor = god:dbref() }
      return descriptor(appearance.render_internal_appearance(bare_ctx))
    end)(),
    -- A thing holds no contents, so rendering over one is deterministically empty.
    thing_context = (function()
      local thing = mux.world.create_object { type = mux.world.types.THING,
        name = 'Parity Bare Thing', location = room }
      local thing_ctx = { object = thing:dbref(), enactor = god:dbref() }
      return descriptor(appearance.render_contents(thing_ctx))
    end)(),
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

local function run()
  return json({
    testing = testing_rows(),
    access_policy = access_policy_rows(),
    object_appearances = appearance_rows(),
  })
end

local commands = {{
  name = 'lua-parity-probe', permission = 'everyone', pattern = '^luaparity$',
  handler = function(ctx)
    emit(ctx.enactor, run())
    return true
  end,
}}
return { commands = commands }
