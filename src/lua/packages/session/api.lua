-- Session snapshots and transactional interactive flow startup.
local native, mux, id = ...
local object = mux.world.object
mux.session = {}

function mux.session.connected_players()
    local players = {}
    for _, p in ipairs(native.connected_players()) do
        players[#players + 1] = {
            object = object(p.dbref),
            name = p.name,
            connected_for = p.connected_for,
            idle_for = p.idle_for,
        }
    end
    return players
end

function mux.session.who_summary()
    return native.who_summary()
end

function mux.session.flow_start(descriptor, module, first_step)
    return native.flow_start(descriptor,module,first_step)
end
