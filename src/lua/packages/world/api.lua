-- Object methods, typed constants, state and world operations.
local native, mux, id = ...
local methods = {}
local object = native.object
native.object_methods(methods)

function methods:dbref() return native.object_id(self) end

function methods:name() return native.get(native.object_id(self), 'name') end

function methods:type() return native.get(native.object_id(self), 'type') end

function methods:description() return native.get(native.object_id(self), 'description') end

function methods:set_name(s) native.set(native.object_id(self), 'name', s) end

function methods:set_description(...) if select('#',...)~=1 then mux.error.raise(mux.error.codes.arg.invalid,'description is required; use nil to clear') end; native.set(native.object_id(self), 'description', ...) end

function methods:internal_description() return native.get(native.object_id(self), 'internal_description') end

function methods:set_internal_description(...) if select('#',...)~=1 then mux.error.raise(mux.error.codes.arg.invalid,'description is required; use nil to clear') end; native.set(native.object_id(self), 'internal_description', ...) end

for _, key in ipairs({'destination', 'home', 'location', 'zone', 'affiliation'}) do
    methods[key] = function(self)
        local n = native.relationship(native.object_id(self), key)
        if n then return object(n) end
    end
end
function methods:lua_parent() return native.relationship(native.object_id(self), 'lua_parent') end
for _, key in ipairs({'destination', 'home', 'zone', 'affiliation', 'lua_parent'}) do
    methods['set_' .. key] = function(self, ...)
        if select('#', ...) ~= 1 then mux.error.raise(mux.error.codes.arg.invalid, 'value is required; supply nil explicitly to clear') end
        native.set_relationship(native.object_id(self), key, ...)
    end
end

function methods:contents(opts)
    opts = opts or {}
    local result = {}
    for _, n in ipairs(native.contents(native.object_id(self), opts)) do
        result[#result + 1] = object(n)
    end
    return result
end

function methods:flags() return native.flag_set(self) end
function methods:powers() return native.power_set(self) end

function methods:state(ns) return native.state(native.object_id(self), ns) end

mux.world = {
    types = native.types,
    flags = native.flags,
    powers = native.powers,
    locks = native.locks,
    object = object,
}

function mux.world.create_object(t)
    return object(native.create(t))
end

function mux.world.list_objects(options)
    local result = {}
    for _, n in ipairs(native.list_objects(options)) do result[#result + 1] = object(n) end
    return result
end

function mux.world.teleport_object(t)
    native.teleport_object(t)
end

function mux.world.pemit(o, s) native.pemit(id(o), s) end

-- Private evaluation accepts canonical keys; public calls require typed constants.
local function lock_identity(value)
    return native.lock_identity(id(value))
end
local function lock_result(t)
    local n = lock_identity(t.object)
    local enactor = lock_identity(t.enactor)
    local subject = lock_identity(t.subject or enactor)
    local cause = lock_identity(t.cause or enactor)
    local path = native.lock_parent(n)
    if not path then return {passes = true} end
    local parent = native.parent(path)
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
    local ok, result = pcall(lock_result, ctx)
    if not ok then native.lock_error(tostring(result)); return false end
    return result.passes
end

function mux.world.destroy_object(object, options) return native.destroy_object(object, options) end
mux.check_db=native.check_db

native.register_lock_result(lock_result)
