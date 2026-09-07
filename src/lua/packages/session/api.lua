-- Session snapshots and transactional interactive flow startup.
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

function mux.session.flow_start(descriptor, module, first_step)
    local ok, failure = pcall(native.flow_start, descriptor, module, first_step)
    if not ok then
        local message = tostring(failure)
        local code = message:match("(connection%.[%w_]+):") or
            message:match("(module%.[%w_]+):") or
            message:match("(unavailable%.[%w_]+):") or "runtime"
        error(mux.error.new({code = code, message = message}), 0)
    end
end
