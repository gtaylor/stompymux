-- Object methods, typed constants, state and world operations.
local native, mux, id = ...
local methods = {}
local mt = {
    __index = methods,
    __eq = function(a, b) return a._id == b._id end,
}

-- Validate the identity before exposing the same object wrapper used by other packages.
local function object(n)
    n = id(n)
    native.get(n, 'name')
    return setmetatable({_id = n}, mt)
end

function methods:dbref() return self._id end

function methods:name() return native.get(self._id, 'name') end

function methods:type() return native.get(self._id, 'type') end

function methods:description() return native.get(self._id, 'description') end

function methods:affiliation()
    local n = native.get(self._id, 'affiliation')
    if n then return object(n) end
end

function methods:set_name(s) native.set(self._id, 'name', s) end

function methods:set_description(s) native.set(self._id, 'description', s) end

function methods:internal_description() return native.get(self._id, 'internal_description') end

function methods:set_internal_description(s) native.set(self._id, 'internal_description', s) end

function methods:set_home(o) native.set(self._id, 'home', id(o)) end

function methods:contents(opts)
    opts = opts or {}
    local result = {}
    for _, n in ipairs(native.contents(self._id, opts.types or {}, id(opts.visible_to))) do
        result[#result + 1] = object(n)
    end
    return result
end

function methods:flags()
    local n = self._id
    return {
        has = function(_, flag) return native.has_flag(n, flag) end,
        add = function(_, flag) return native.flag(n, flag, true) end,
        remove = function(_, flag) return native.flag(n, flag, false) end,
    }
end

function methods:powers()
    local n = self._id
    return {
        has = function(_, power) return native.has_power(n, power) end,
        add = function(_, power) return native.power(n, power, true) end,
        remove = function(_, power) return native.power(n, power, false) end,
    }
end

function methods:state(ns) return native.state(self._id, ns) end

mux.world = {
    types = {ROOM = 0, THING = 1, EXIT = 2, PLAYER = 3},
    flags = native.flags,
    powers = native.powers,
    locks = native.locks,
    object = object,
}

function mux.world.create_object(t)
    local copy = {}
    for k, v in pairs(t) do copy[k] = id(v) end
    return object(native.create(copy))
end

function mux.world.teleport_object(t)
    native.set(id(t.object), 'location', id(t.destination))
end

function mux.world.pemit(o, s) native.pemit(id(o), s) end

-- Private evaluation accepts canonical keys; public calls require typed constants.
local function lock_identity(value)
    if type(value) == 'table' then value = value._id end
    return native.lock_identity(value)
end
function mux.world._lock_result(t)
    local n = lock_identity(t.object)
    local enactor = lock_identity(t.enactor)
    local subject = lock_identity(t.subject or enactor)
    local cause = lock_identity(t.cause or enactor)
    local path = native.lock_parent(n)
    if not path then return {passes = true} end
    local parent = _parents[path]
    if not parent then error('Missing lock parent') end
    if parent.locks == nil then return {passes = true} end
    if type(parent.locks) ~= 'table' then error('locks must be a table') end
    local lock = parent.locks[t.lock]
    if lock == nil then return {passes = true} end
    if type(lock) ~= 'function' then error('lock handler must be a function') end
    return native.evaluate_lock(lock, {
        object = n, subject = subject, enactor = enactor, cause = cause,
        source = id(t.source), destination = id(t.destination),
        descriptor = t.descriptor, lock = t.lock, silent = t.silent or false,
        args = {},
    })
end
function mux.world.lock_passes(t)
    if type(t) ~= 'table' then error('lock options must be a table') end
    local allowed = {object=true,enactor=true,lock=true,cause=true,subject=true}
    for k in pairs(t) do if not allowed[k] then error('unknown lock option: '..tostring(k)) end end
    local ctx = {object=lock_identity(t.object), enactor=lock_identity(t.enactor), lock=native.lock_key(t.lock), silent=true, descriptor=native.callback_descriptor()}
    ctx.subject=lock_identity(t.subject or ctx.enactor)
    ctx.cause=lock_identity(t.cause or ctx.enactor)
    local ok, result = pcall(mux.world._lock_result, ctx)
    if not ok then native.lock_error(tostring(result)); return false end
    return result.passes
end
