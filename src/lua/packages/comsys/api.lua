-- Channel creation and metadata compatibility facade.
local native, mux, id = ...
mux.comsys = {flags = {PUBLIC = 1}}

function mux.comsys.create_channel(name)
    native.channel(name)
    return {
        set_object = function(_, o) native.channel(name, id(o)) end,
        flags = function()
            return {
                add = function(_, f) native.channel(name, nil, f) end,
            }
        end,
    }
end
