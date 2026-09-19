-- Differential probe: mux.text styled-text utilities.
-- Oracle: btmux-khi/src/mux/lua/packages/mux/text/mux_text_bindings.c.
-- C-compatible corpus lives in c_* keys; documented Rust extensions
-- (AD-TEXT-EXTENSIONS-001: Unicode width, Markdown documents) in extension_* keys.
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

local function returns(...)
  local result = { count = select('#', ...), values = {} }
  for index = 1, result.count do
    result.values[index] = descriptor(select(index, ...))
  end
  return result
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
  local corpus = {
    'hello', '', 'a b c', '[bold]styled[/]plain', '[fg=red]color[/]',
    '[bold][underline]deep[/][/]', 'unclosed [bold]tag', 'a\1b', 'a\31b',
    '\127delete', 'tab\there', 'quote"in', "single'", 'percent%s', 'a\nb',
  }
  local widths, truncates, strips, printables, markups = {}, {}, {}, {}, {}
  for index, text in ipairs(corpus) do
    widths[index] = returns(mux.text.width(text))
    strips[index] = returns(mux.text.strip_style(text))
    printables[index] = returns(mux.text.is_printable_ascii(text))
    markups[index] = capture(function() return mux.text.markup(text) end)
    truncates[index] = {
      zero = capture(function() return mux.text.truncate(text, 0) end),
      one = capture(function() return mux.text.truncate(text, 1) end),
      exact = capture(function() return mux.text.truncate(text, 7) end),
      huge = capture(function() return mux.text.truncate(text, 100) end),
    }
  end
  return json({
    c_width = widths,
    c_truncate = truncates,
    c_strip_style = strips,
    c_is_printable_ascii = printables,
    c_markup = markups,
    -- Non-UTF-8 byte corpus: C's renderer replaces each byte that does not
    -- begin a valid sequence with one U+FFFD (styled_append_utf8_codepoint)
    -- for width/strip_style, while styled_text_truncate stops at the first
    -- such byte without substituting anything.
    c_invalid_utf8 = (function()
      local invalid = {
        'lone\255', 'run\255\254', 'mixed a\255b',
        'truncated3 \224\160', 'truncated4 \240\159\152',
        'valid then cut \195\169\195', 'surrogate \237\160\128',
        'overlong \192\175', 'lone continuation \128',
        'markup [bold]\255[/]', 'before \255[bold]x[/]', 'bad tag [\255]x[/]',
        'escape \027[1m\255 after',
      }
      local rows = {}
      for index, text in ipairs(invalid) do
        rows[index] = {
          width = returns(mux.text.width(text)),
          strip = returns(mux.text.strip_style(text)),
          truncate_three = capture(function() return mux.text.truncate(text, 3) end),
          truncate_huge = capture(function() return mux.text.truncate(text, 100) end),
        }
      end
      return rows
    end)(),
    c_style = {
      empty_options = returns(mux.text.style('x', {})),
      nil_options = capture(function() return mux.text.style('x', nil) end),
      fg = returns(mux.text.style('val', { foreground = 'red' })),
      bg = returns(mux.text.style('val', { background = 'blue' })),
      bold = returns(mux.text.style('val', { bold = true })),
      underline_inverse = returns(mux.text.style('val', { underline = true, inverse = true })),
      all = returns(mux.text.style('val', {
        foreground = 'red', background = 'blue', bold = true, underline = true, inverse = true,
      })),
      false_booleans = returns(mux.text.style('val', { bold = false, underline = false })),
      number_fg = capture(function() return mux.text.style('x', { foreground = 12 }) end),
      bracket_fg = capture(function() return mux.text.style('x', { foreground = '[red]' }) end),
      boolean_fg = capture(function() return mux.text.style('x', { foreground = true }) end),
      number_bold = capture(function() return mux.text.style('x', { bold = 1 }) end),
      unknown_style_color = capture(function() return mux.text.style('x', { foreground = 'no_such_color' }) end),
      nul_value = capture(function() return mux.text.style('x\0y', {}) end),
      omitted_value = capture(function() return mux.text.style() end),
      false_value = capture(function() local f = mux.text.style return f(false, {}) end),
      number_value = capture(function() return mux.text.style(42, {}) end),
      extra_args = returns(mux.text.style('x', {}, 'ignored')),
    },
    c_edges = {
      width_number = capture(function() return mux.text.width(42) end),
      width_omitted = capture(function() return mux.text.width() end),
      width_false = capture(function() local f = mux.text.width return f(false) end),
      width_extra = capture(function() return mux.text.width('ab', 'ignored') end),
      truncate_negative = capture(function() return mux.text.truncate('abc', -1) end),
      truncate_float = capture(function() return mux.text.truncate('abcdef', 2.9) end),
      truncate_float_down = capture(function() return mux.text.truncate('abcdef', -0.5) end),
      truncate_string_width = capture(function() return mux.text.truncate('abcdef', '2') end),
      truncate_nonnumeric_string = capture(function() return mux.text.truncate('abc', 'wide') end),
      truncate_boolean = capture(function() return mux.text.truncate('abc', false) end),
      truncate_omitted_width = capture(function() return mux.text.truncate('abc') end),
      truncate_nil_width = capture(function() return mux.text.truncate('abc', nil) end),
      truncate_number_value = capture(function() return mux.text.truncate(12345, 2) end),
      truncate_extra = capture(function() return mux.text.truncate('abc', 2, 'ignored') end),
      printable_omitted = capture(function() return mux.text.is_printable_ascii() end),
      printable_number = capture(function() return mux.text.is_printable_ascii(42) end),
      printable_boolean = capture(function() return mux.text.is_printable_ascii(true) end),
      printable_table = capture(function() return mux.text.is_printable_ascii({}) end),
      printable_extra = capture(function() return mux.text.is_printable_ascii('a', 'ignored') end),
      printable_boundaries = returns(
        mux.text.is_printable_ascii(string.char(0x20)),
        mux.text.is_printable_ascii(string.char(0x7e)),
        mux.text.is_printable_ascii(string.char(0x1f)),
        mux.text.is_printable_ascii(string.char(0x7f)),
        mux.text.is_printable_ascii(string.char(0xff))
      ),
      markup_number = capture(function() return mux.text.markup(42) end),
      markup_omitted = capture(function() return mux.text.markup() end),
      markup_extra = capture(function() return mux.text.markup('ok', 'ignored') end),
      markup_unknown_tag = capture(function() return mux.text.markup('[fancy]x[/]') end),
      strip_number = capture(function() return mux.text.strip_style(42) end),
      strip_omitted = capture(function() return mux.text.strip_style() end),
      strip_extra = capture(function() return mux.text.strip_style('ab', 'ignored') end),
      style_options_number = capture(function() return mux.text.style('x', 12) end),
    },
    extension_unicode_width = returns(
      mux.text.width('h\xc3\xa9llo'),
      mux.text.width('\xe2\x9c\x93'),
      mux.text.truncate('h\xc3\xa9llo', 2)
    ),
    -- Documented Rust extension (AD-TEXT-EXTENSIONS-001): record only when the
    -- markdown surface exists so C omits the key entirely.
    extension_markdown = (type(mux.text.markdown) == 'function') and {
      document_type = descriptor(type(mux.text.markdown('**paired**'))),
    } or {},
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
