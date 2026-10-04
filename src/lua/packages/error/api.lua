-- Structured failures preserve Lua values and all protected-call results.
local native, traceback, getinfo = ...
local api = {}

local function call_context()
    return native.capture()
end

local function type_name(value, present)
    return present and type(value) or 'no value'
end

local function typeerror(context, argument, expected, value, present)
    error(context.where.."bad argument #"..argument.." to '"..context.name..
        "' ("..expected.." expected, got "..type_name(value,present)..")",0)
end

local function cstring(value)
    if type(value) == 'number' then value = tostring(value) end
    if type(value) ~= 'string' then return nil end
    local nul=value:find('\0',1,true)
    return nul and value:sub(1,nul-1) or value
end

local function code(value, argument, table_argument, context, present)
    if type(value) == 'table' then
        local field=value.code
        if cstring(field)==nil then
            if context.method and table_argument>0 then table_argument=table_argument-1 end
            typeerror(context,table_argument,'string',field,true)
        end
        value=field
    end
    local checked=cstring(value)
    if type(checked) ~= 'string' then
        typeerror(context,argument,'string',value,present)
    end
    return checked
end

local mt = {__tostring = function(e)
    return (cstring(e.code) or 'lua.error') .. ': ' .. (cstring(e.message) or 'unknown Lua error')
end}

function api.is(...)
    local context=call_context()
    local count=select('#',...)
    local value,expected=...
    expected=code(expected,2,count+2,context,count>=2)
    local actual=type(value)=='table' and cstring(value.code) or nil
    if actual then actual=actual:sub(1,255) end
    return actual ~= nil and
        (actual == expected or actual:sub(1, #expected + 1) == expected .. '.')
end

mt.__index = {is = api.is, root = function(e)
    local depth = 0
    while depth < 64 and type(e) == 'table' and type(e.cause) == 'table' do
        depth=depth+1
        e = e.cause
    end
    return e
end}

function api.new(...)
    local context=call_context()
    local count=select('#',...)
    local fields=...
    if type(fields)~='table' then typeerror(context,1,'table',fields,count>=1) end
    local checked_code=code(fields.code,count+1,count+3,context,true)
    local raw_message=fields.message
    local message = cstring(raw_message)
    if type(message)~='string' then typeerror(context,count+3,'string',raw_message,true) end
    return setmetatable({code=checked_code, message=message,
        detail=fields.detail, cause=fields.cause}, mt)
end

local function normalize(err)
    if type(err) == 'table' and cstring(err.code) then return err end
    local text
    if type(err)=='table' then
        local c,m=cstring(err.code),cstring(err.message)
        text=c and m and (c:sub(1,255)..': '..m:sub(1,2047)) or 'table'
    elseif type(err)=='string' or type(err)=='number' then
        text=cstring(err) --[[@as string]]
        local source,line,detail=text:match('^%[string "(.-)"%]:(%d+):(.*)$')
        if source then text=source..':'..line..':'..detail end
    else
        text=type(err)
    end
    return api.new({code='mux.runtime', message=text:sub(1,2047)})
end

function api.raise(...)
    local context=call_context()
    local count=select('#',...)
    local c,message,detail=...
    local raw_c=c
    c=code(c,1,count+2,context,count>=1)
    -- C's check_code pushes the table's code field to absolute index 3, so an
    -- omitted detail slot reads the pushed field back; with a plain string
    -- code nothing is pushed, so lua_error_push lands the new error table at
    -- index 3 and the omitted detail slot aliases the error itself. A
    -- provided detail still occupies index 3 and is used unchanged.
    local detail_from_slot=type(raw_c)=='table' and count<3
    local detail_is_self=count<3 and not detail_from_slot
    if detail_from_slot then detail=raw_c.code end
    local raw_message=message
    message=cstring(raw_message)
    if type(message)~='string' then typeerror(context,2,'string',raw_message,count>=2) end
    local raised=api.new({code=c,message=message,detail=detail})
    if detail_is_self then raised.detail=raised end
    error(raised,0)
end

function api.check(value, err) if not value then error(err,0) end return value end

function api.wrap(...)
    local context=call_context()
    local count=select('#',...)
    local err,c,message=...
    local raw_c=c
    c=code(c,2,count+2,context,count>=2)
    -- C's check_code pushes a table's code field to index 3, so an omitted
    -- message is read back from the pushed field itself.
    if type(raw_c)=='table' and count<3 then message=raw_c.code end
    local raw_message=message
    message=cstring(raw_message)
    if type(message)~='string' then typeerror(context,3,'string',raw_message,count>=3) end
    return api.new({code=c,message=message,cause=normalize(err)})
end

local function pack(...) return {n=select('#',...),...} end
local function native_traceback(trace)
    if type(trace)~='string' then return trace end
    -- Preserve LuaJIT's original first line, then rebuild only authored Lua frames
    -- from its debug metadata. mlua contributes facade/host frames with Rust sources
    -- which the reference runtime's direct lua_call boundary does not expose.
    local message=trace:match('^([^\n]*)') or trace
    local frames={}
    local level=2
    while level<64 do
        local info=getinfo(level,'Sln')
        if not info then break end
        local source=info.short_src or '?'
        if source:find('.lua',1,true) then frames[#frames+1]=info end
        level=level+1
    end
    local lines={message,'stack traceback:'}
    for index,info in ipairs(frames) do
        local source=info.short_src or '?'
        local line=info.currentline or 0
        if index==#frames then
            lines[#lines+1]='\t'..source..':'..line..': in function <'..source..':'..(info.linedefined or line)..'>'
        elseif info.name then
            lines[#lines+1]='\t'..source..':'..line..": in function '"..info.name.."'"
        else
            lines[#lines+1]='\t'..source..':'..line..': in main chunk'
        end
    end
    return table.concat(lines,'\n')
end
function api.pcall(...)
    local context=call_context()
    local count=select('#',...)
    local fn=...
    if type(fn)~='function' then typeerror(context,1,'function',fn,count>=1) end
    local arguments={n=count-1,select(2,...)}
    local result=pack(pcall(fn,unpack(arguments,1,arguments.n)))
    if result[1] then return unpack(result,1,result.n) end
    local original=native.convert(result[2])
    local trace_ok,trace=pcall(traceback,original,2)
    local err=normalize(original)
    if trace_ok then err.traceback=native_traceback(trace) else err.traceback='Lua traceback unavailable' end
    return false,err
end

api.code_tree=native.code_tree
api.namespace=native.namespace
api.codes=api.code_tree('mux')
return api
