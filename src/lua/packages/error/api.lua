-- Structured failures preserve Lua values and all protected-call results.
local native, traceback = ...
local api = {}
local function code(value)
    if type(value) == 'userdata' then value = value.code end
    if type(value) == 'number' then value = tostring(value) end
    assert(type(value) == 'string', 'expected an error code')
    return value
end
local mt = {__tostring = function(e) return e.code .. ': ' .. e.message end}
function api.is(value, expected)
    expected = code(expected)
    return type(value) == 'table' and type(value.code) == 'string' and
        (value.code == expected or value.code:sub(1, #expected + 1) == expected .. '.')
end
mt.__index = {is = api.is, root = function(e)
    local seen, depth = {},0
    while depth < 64 and type(e.cause) == 'table' and not seen[e.cause] do
        depth=depth+1
        seen[e] = true
        e = e.cause
    end
    return e
end}
function api.new(fields)
    assert(type(fields) == 'table', 'expected error options')
    local message = fields.message
    if type(message) == 'number' then message = tostring(message) end
    assert(type(message) == 'string', 'expected error message')
    return setmetatable({code=code(fields.code), message=message,
        detail=fields.detail, cause=fields.cause}, mt)
end
local function normalize(err)
    if type(err) == 'table' and type(err.code) == 'string' then return err end
    local ok, text = pcall(tostring, err)
    return api.new({code='mux.runtime', message=ok and text:sub(1,2047) or 'Unable to describe Lua failure'})
end
function api.raise(c, message, detail) error(api.new({code=c,message=message,detail=detail}),0) end
function api.check(value, err) if not value then error(err,0) end return value end
function api.wrap(err, c, message) return api.new({code=c,message=message,cause=normalize(err)}) end
local function pack(...) return {n=select('#',...),...} end
function api.pcall(fn, ...)
    assert(type(fn)=='function','expected a function')
    local result=pack(pcall(fn,...))
    if result[1] then return unpack(result,1,result.n) end
    local err=normalize(result[2])
    local ok, trace=pcall(traceback,'',2)
    if ok then err.traceback=trace end
    return false,err
end
api.code_tree=native.code_tree
api.namespace=native.namespace
api.codes=api.code_tree('mux')
return api
