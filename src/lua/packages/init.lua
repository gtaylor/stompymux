-- Shared object identity conversion used by embedded package facades.
local function id(o)
    if type(o) == 'table' then return o._id else return o end
end

return id
