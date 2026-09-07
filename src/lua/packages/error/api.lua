local api = {}
local function code(value)
    if type(value) == "userdata" then value = value.code end
    assert(type(value) == "string" and value ~= "", "expected an error code")
    return value
end
function api.is(value, expected)
    expected = code(expected)
    return type(value) == "table" and type(value.code) == "string" and
        (value.code == expected or value.code:sub(1, #expected + 1) == expected .. ".")
end
local mt = {__tostring = function(e) return e.code .. ": " .. e.message end}
mt.__index = {is = api.is, root = function(e)
    for _ = 1, 64 do
        if type(e.cause) ~= "table" then break end
        e = e.cause
    end
    return e
end}
function api.new(options)
    assert(type(options) == "table", "expected error options")
    local result = {code = code(options.code), message = options.message,
        detail = options.detail, cause = options.cause}
    assert(type(result.message) == "string", "expected error message")
    return setmetatable(result, mt)
end
function api.wrap(cause, expected, message)
    return api.new({code=expected, message=message, cause=cause})
end
return api
