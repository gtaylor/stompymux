-- Private conversion validates generation-checked object handles.
local native=...
return function(value)
    if value == nil then return nil end
    return native.object_id(value)
end
