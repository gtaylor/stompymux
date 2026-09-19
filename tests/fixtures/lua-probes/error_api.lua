local function hex(s) return (s:gsub('.',function(c) return string.format('%02x',string.byte(c)) end)) end
-- Messages and tracebacks embed the running server's incidental source
-- spelling: C names the isolated game's absolute (LuaJIT-truncated) probe
-- path while Rust names the chunk. Reduce every spelling of this probe file
-- to the bare file name so the compared bytes keep line numbers and frames.
local function probe_source(text)
  if type(text)~='string' then return text end
  return (text
    :gsub('%[string "([^"]+)"%]', '%1')
    :gsub('[^%s:]*lua_parity_probe%.lua', 'lua_parity_probe.lua'))
end
local function desc(ok,v)
  local t=type(v)
  local r={ok=ok,type=t}
  if t=='string' then r.bytes=hex(v)
  elseif t=='table' then
    r.code=type(v.code)=='string' and hex(v.code) or type(v.code)
    r.message=type(v.message)=='string' and hex(probe_source(v.message)) or type(v.message)
    r.traceback=type(v.traceback)=='string' and hex(probe_source(v.traceback)) or type(v.traceback)
  else r.text=hex(tostring(v)) end
  return r
end
local function capture(fn,...)
 local r={pcall(fn,...)} return desc(r[1],r[2])
end
local function esc(s) return '"'..s:gsub('[\\"]',function(c)return '\\'..c end)..'"' end
local function json(v)
 if type(v)=='boolean' or type(v)=='number' then return tostring(v) end
 if type(v)=='string' then return esc(v) end
 local keys={} for k in pairs(v) do keys[#keys+1]=k end table.sort(keys)
 local out={} for _,k in ipairs(keys) do out[#out+1]=esc(k)..':'..json(v[k]) end
 return '{'..table.concat(out,',')..'}'
end
local function run()
 local touched=0
 local cause=setmetatable({}, {__tostring=function() touched=touched+1; return 'cause' end})
 local badwrap=capture(mux.error.wrap,cause,{}, {})
 local result={
   new_nil=capture(mux.error.new,nil),
   new_both_bad=capture(mux.error.new,{code={},message={}}),
   new_message_bad=capture(mux.error.new,{code='x',message={}}),
   raise_bad=capture(mux.error.raise,{},{}),
   is_bad=capture(mux.error.is,{},{}),
   pcall_bad=capture(mux.error.pcall,false),
   pcall_boom={mux.error.pcall(function() error('boom') end)},
   wrap_bad=badwrap,
   wrap_touched=touched,
 }
 local p=result.pcall_boom result.pcall_boom=desc(p[1],p[2])
 -- C lua_mux_error_raise reads the detail slot at absolute index 3, which a
 -- two-argument string-code raise leaves occupied by the freshly pushed error
 -- table itself (lua_error_push lands it there), so detail aliases the error.
 local raise_ok,raise_err=pcall(mux.error.raise,'parity.code','raised')
 result.raise_string_detail={
   self_detail=desc(type(raise_err)=='table' and raise_err.detail==raise_err or false),
   detail_type=desc(type(raise_err)=='table' and type(raise_err.detail) or 'no table'),
 }
 return json(result)
end
local function emit(target,payload) mux.world.pemit(target,'LUA_PARITY:1/1:'..payload) end
return {commands={{pattern='^luaparity$',handler=function(ctx) emit(ctx.enactor,run()); return true end}}}
