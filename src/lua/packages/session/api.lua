-- Session snapshots and the existing deferred-flow error.
local native, mux, id = ...
local object = mux.world.object
mux.session = {}

function mux.session.connected_players()
    local players = {}
    for _, p in ipairs(_connected_players or {}) do
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
    return _who_summary or {hidden = 0, record = 0}
end

function mux.session.flow_start()
    error('Interactive Lua flows are unavailable in this milestone')
end
