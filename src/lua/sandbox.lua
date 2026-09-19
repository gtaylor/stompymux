-- Remove Lua's mutable loader tables and runtime control libraries after the
-- host has installed all built-in facades.
local package_table = package
local forbidden = {
    io = true,
    os = true,
    debug = true,
    package = true,
    coroutine = true,
    jit = true,
    ffi = true,
}
for name in pairs(forbidden) do
    package_table.loaded[name] = nil
    package_table.preload[name] = nil
end

package = nil
