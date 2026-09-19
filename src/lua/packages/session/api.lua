-- Session snapshots and transactional interactive flow startup.
local native, mux, id = ...
mux.session = {}

-- C builds each record (object handle, name, connected_for, idle_for) natively;
-- the records table is passed through unchanged.
function mux.session.connected_players()
    return native.connected_players()
end

function mux.session.who_summary()
    return native.who_summary()
end

function mux.session.flow_start(...)
    -- Forward the exact argument list so omitted arguments stay omitted.
    return native.flow_start(...)
end
