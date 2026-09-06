-- Restrict require to game packages and safe built-ins. Do not allow scripts to
-- recover removed runtime libraries through package.loaded or preload.
local original_require = require
local forbidden = {io = true, os = true, debug = true, ffi = true, jit = true}
for name in pairs(forbidden) do
    package.loaded[name] = nil
    package.preload[name] = nil
end

function require(name)
    if forbidden[name:match('^[^.]+')] or not name:match('^[%w_%.]+$') or name:find('%.%.') then
        error('Unavailable Lua package ' .. tostring(name))
    end
    return original_require(name)
end

package.loadlib = nil
