//! LuaLS contract blocks for the mux world surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00076
//|---Adds a flag and reports whether the object changed.
//|---@param flag Flag Checked constant from [`mux.world.flags`](lua://mux.world.flags).
//|---@return boolean changed
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.flag.invalid`](lua://mux.error.codes.flag.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.flag.invalid
//|---@see mux.error.codes.object.unavailable
//|function Flags:add(flag) end
// lua-types-end

// lua-types-begin mux 00077
//|---Tests whether this object has a flag.
//|---@param flag Flag Checked constant from [`mux.world.flags`](lua://mux.world.flags).
//|---@return boolean present
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.flag.invalid`](lua://mux.error.codes.flag.invalid).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.flag.invalid
//|function Flags:has(flag) end
// lua-types-end

// lua-types-begin mux 00078
//|---Lists set flags in native registry order.
//|---@return Flag[] values
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.object.invalid
//|function Flags:list() end
// lua-types-end

// lua-types-begin mux 00079
//|---Removes a flag and reports whether the object changed.
//|---@param flag Flag Checked constant from [`mux.world.flags`](lua://mux.world.flags).
//|---@return boolean changed
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.flag.invalid`](lua://mux.error.codes.flag.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.flag.invalid
//|---@see mux.error.codes.object.unavailable
//|function Flags:remove(flag) end
// lua-types-end

// lua-types-begin mux 00080
//|---Returns this object's assigned affiliation, or nil when none is assigned or the affiliate is being destroyed.
//|---@return Object? affiliation Assigned affiliation.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) for an invalid receiver or stored affiliation.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|function Object:affiliation() end
// lua-types-end

// lua-types-begin mux 00081
//|---Returns matching objects directly contained by or attached to this object.
//|---@param options? ContentsOptions Optional type and visibility filters.
//|---@return Object[] contents
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|function Object:contents(options) end
// lua-types-end

// lua-types-begin mux 00082
//|---Returns this object's native database reference.
//|---@return DbRef dbref
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.object.invalid
//|function Object:dbref() end
// lua-types-end

// lua-types-begin mux 00083
//|---Returns this object's styled description, or nil when it is unset.
//|---@return string? description
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
//|---for a stale Object.
//|---@see mux.error.codes.object.invalid
//|function Object:description() end
// lua-types-end

// lua-types-begin mux 00084
//|---Returns this exit's destination, or nil when it is unlinked or the
//|---destination is being destroyed.
//|---@return Object? destination
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
//|---during `@lua/check`, or
//|---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
//|---the receiver is not an exit or its stored destination is invalid.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|function Object:destination() end
// lua-types-end

// lua-types-begin mux 00085
//|---Creates a handle for this object's flags.
//|---@return Flags flags
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.unavailable.checking
//|function Object:flags() end
// lua-types-end

// lua-types-begin mux 00086
//|---Returns this thing or player's home, or nil when no home is assigned or the
//|---home is being destroyed.
//|---@return Object? home
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
//|---during `@lua/check`, or
//|---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
//|---the receiver is not a thing or player or its stored home is invalid.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|function Object:home() end
// lua-types-end

// lua-types-begin mux 00087
//|---Returns this object's styled internal description, or nil when it is unset.
//|---@return string? description
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
//|---for a stale Object.
//|---@see mux.error.codes.object.invalid
//|function Object:internal_description() end
// lua-types-end

// lua-types-begin mux 00088
//|---Returns this thing or player's current location, or nil when no location is
//|---assigned or the location is being destroyed.
//|---@return Object? location
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
//|---during `@lua/check`, or
//|---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
//|---the receiver is not a thing or player or its stored location is invalid.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|function Object:location() end
// lua-types-end

// lua-types-begin mux 00089
//|---Returns this object's direct Lua parent path, or nil when none is assigned.
//|---@return string? parent `object_logic`-relative parent path.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|function Object:lua_parent() end
// lua-types-end

// lua-types-begin mux 00090
//|---Returns this object's current stored name.
//|---@return string name
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.object.invalid
//|function Object:name() end
// lua-types-end

// lua-types-begin mux 00091
//|---Creates a handle for this object's powers.
//|---@return Powers powers
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.unavailable.checking
//|function Object:powers() end
// lua-types-end

// lua-types-begin mux 00092
//|---Assigns this object's affiliation, or clears it when `affiliation` is nil.
//|---@param affiliation DbRef|Object|nil Any live object to assign, or nil to clear the affiliation. This argument must be supplied explicitly.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `affiliation` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) for an invalid reference, or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable) when either object is being destroyed.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function Object:set_affiliation(affiliation) end
// lua-types-end

// lua-types-begin mux 00093
//|---Sets this object's styled description. Nil or an empty string clears it.
//|---@param description string|nil Valid UTF-8 styled-text markup without embedded NUL bytes, or nil to clear the description. This argument must be supplied explicitly.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
//|---during `@lua/check`, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
//|---for a stale Object,
//|---[`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable)
//|---when the object is being destroyed, or
//|---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for text
//|---that is too long or has invalid UTF-8 or styled-text markup.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|---@see mux.error.codes.arg.invalid
//|function Object:set_description(description) end
// lua-types-end

// lua-types-begin mux 00094
//|---Sets this exit's destination, or clears it when `destination` is nil.
//|---@param destination DbRef|Object|nil Live object capable of containing objects, or nil to unlink this exit. This argument must be supplied explicitly.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `destination` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when the receiver is not an exit or the destination cannot contain objects, or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable) when the receiver or destination is being destroyed.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function Object:set_destination(destination) end
// lua-types-end

// lua-types-begin mux 00095
//|---Sets this thing or player's home to a live object capable of containing
//|---objects.
//|---@param new_home DbRef|Object Live room, thing, or player to assign as the object's home.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
//|---during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid)
//|---when `new_home` is omitted,
//|---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
//|---the receiver is not a thing or player, the home cannot contain objects, or
//|---the object would be its own home, or
//|---[`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable)
//|---when the receiver or home is being destroyed.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function Object:set_home(new_home) end
// lua-types-end

// lua-types-begin mux 00096
//|---Sets this object's styled internal description. Nil or an empty string clears it.
//|---@param description string|nil Valid UTF-8 styled-text markup without embedded NUL bytes, or nil to clear the internal description. This argument must be supplied explicitly.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
//|---during `@lua/check`, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
//|---for a stale Object,
//|---[`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable)
//|---when the object is being destroyed, or
//|---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for text
//|---that is too long or has invalid UTF-8 or styled-text markup.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|---@see mux.error.codes.arg.invalid
//|function Object:set_internal_description(description) end
// lua-types-end

// lua-types-begin mux 00097
//|---Assigns this object's direct Lua parent path, or clears it when `parent` is nil.
//|---@param parent string|nil Existing `object_logic`-relative `.lua` path, or nil to clear it. This argument must be supplied explicitly.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `parent` is omitted or malformed, [`mux.error.codes.module.invalid`](lua://mux.error.codes.module.invalid) for an invalid or unavailable path, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.module.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function Object:set_lua_parent(parent) end
// lua-types-end

// lua-types-begin mux 00098
//|---Changes this object's name using native object-name validation.
//|---@param name string New UTF-8 name, optionally containing styled-text markup.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function Object:set_name(name) end
// lua-types-end

// lua-types-begin mux 00099
//|---Assigns this object's zone, or clears it when `zone` is nil.
//|---@param zone DbRef|Object|nil Live thing or room to assign, or nil to clear the zone. This argument must be supplied explicitly.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `zone` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function Object:set_zone(zone) end
// lua-types-end

// lua-types-begin mux 00100
//|---Creates a persistent-state handle for an exact, case-sensitive namespace.
//|---@param namespace string
//|---@return State state
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.state.invalid
//|function Object:state(namespace) end
// lua-types-end

// lua-types-begin mux 00101
//|---Returns this object's native object type.
//|---@return ObjectType? type
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.object.invalid
//|function Object:type() end
// lua-types-end

// lua-types-begin mux 00102
//|---Returns this object's assigned zone, or nil when no zone is assigned or the zone is being destroyed.
//|---@return Object? zone Assigned zone.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|function Object:zone() end
// lua-types-end

// lua-types-begin mux 00103
//|---Grants a power and reports whether the object changed.
//|---@param power Power Checked constant from [`mux.world.powers`](lua://mux.world.powers).
//|---@return boolean changed
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), or [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.power.invalid
//|function Powers:add(power) end
// lua-types-end

// lua-types-begin mux 00104
//|---Tests whether this object has a power.
//|---@param power Power Checked constant from [`mux.world.powers`](lua://mux.world.powers).
//|---@return boolean present
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.power.invalid
//|function Powers:has(power) end
// lua-types-end

// lua-types-begin mux 00105
//|---Lists granted powers in native registry order.
//|---@return Power[] values
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.object.invalid
//|function Powers:list() end
// lua-types-end

// lua-types-begin mux 00106
//|---Removes a power and reports whether the object changed.
//|---@param power Power Checked constant from [`mux.world.powers`](lua://mux.world.powers).
//|---@return boolean changed
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), or [`mux.error.codes.power.invalid`](lua://mux.error.codes.power.invalid).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.power.invalid
//|function Powers:remove(power) end
// lua-types-end

// lua-types-begin mux 00107
//|---Deletes a state key and reports whether it existed.
//|---@param key string
//|---@return boolean existed
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.state.invalid
//|function State:delete(key) end
// lua-types-end

// lua-types-begin mux 00108
//|---Lists key/value records sorted by key.
//|---@return StateEntry[] entries
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.unavailable`](lua://mux.error.codes.state.unavailable).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.state.unavailable
//|function State:entries() end
// lua-types-end

// lua-types-begin mux 00109
//|---Gets a stored value, an optional default, or nil.
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) or [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
//|---@generic T
//|---@param key string
//|---@param default? T
//|---@return StateValue|T|nil value
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.state.invalid
//|function State:get(key, default) end
// lua-types-end

// lua-types-begin mux 00110
//|---Returns only the requested keys that are present.
//|---@param keys string[]
//|---@return table<string, StateValue> values
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.state.invalid
//|function State:get_many(keys) end
// lua-types-end

// lua-types-begin mux 00111
//|---Tests whether a state key is present.
//|---@param key string
//|---@return boolean exists
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.state.invalid
//|function State:has(key) end
// lua-types-end

// lua-types-begin mux 00112
//|---Lists keys sorted in native key order.
//|---
//|---Raises [`mux.error.codes.state.unavailable`](lua://mux.error.codes.state.unavailable) outside a callback transaction or if state changes while enumerating.
//|---@return string[] keys
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.state.unavailable
//|function State:keys() end
// lua-types-end

// lua-types-begin mux 00113
//|---Sets a supported value, or deletes the key when `value` is nil.
//|---The `value` argument is required; omission raises
//|---[`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid).
//|---
//|---Raises invalid-object/key/value errors or [`mux.error.codes.state.value_too_large`](lua://mux.error.codes.state.value_too_large).
//|---@param key string
//|---@param value StateValue|nil
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.state.invalid
//|---@see mux.error.codes.state.value_too_large
//|function State:set(key, value) end
// lua-types-end

// lua-types-begin mux 00114
//|---Applies several persistent state updates. Use `State:set` or `State:delete` for removals.
//|---@param values table<string, StateValue>
//|---
//|---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.state.invalid`](lua://mux.error.codes.state.invalid), [`mux.error.codes.state.value_too_large`](lua://mux.error.codes.state.value_too_large).
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.state.invalid
//|---@see mux.error.codes.state.value_too_large
//|function State:set_many(values) end
// lua-types-end

// lua-types-begin mux 00141
//|---Creates a room, thing, or exit selected by a typed object-kind constant.
//|---Rooms are detached; things require a container and receive a home; exits
//|---require a source and may be linked to a destination. Unknown fields and
//|---fields that do not apply to the selected type are rejected.
//|---@param options CreateObjectOptions Exact creation fields selected by `options.type`.
//|---@return Object object Newly created object.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable), or [`mux.error.codes.internal`](lua://mux.error.codes.internal) if a validated object kind reaches an unsupported native creation branch.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|---@see mux.error.codes.internal
//|function mux_world.create_object(options) end
// lua-types-end

// lua-types-begin mux 00142
//|---Silently schedules a live object for destruction by the normal maintenance purge.
//|---@param object DbRef|Object Object to destroy.
//|---@param options? DestroyOptions Destruction controls; unknown fields are rejected.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable), or [`mux.error.codes.internal`](lua://mux.error.codes.internal) for an unexpected native destruction result.
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|---@see mux.error.codes.internal
//|function mux_world.destroy_object(object, options) end
// lua-types-end

// lua-types-begin mux 00143
//|---Lists database objects matching optional type and direct-zone filters.
//|---@param options? ListObjectsOptions Optional filters; unknown fields are rejected.
//|---@return Object[] objects Matching objects in ascending dbref order.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function mux_world.list_objects(options) end
// lua-types-end

// lua-types-begin mux 00144
//|---Tests a native object lock without emitting lock messages or performing the
//|---associated action. The lock runs with a silent callback context.
//|---@param options LockPassesOptions Lock invocation fields; unknown fields are rejected.
//|---@return boolean passes Whether the selected lock passes.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function mux_world.lock_passes(options) end
// lua-types-end

// lua-types-begin mux 00145
//|---Creates a validated object handle from a dbref or existing handle.
//|---@param dbref DbRef|Object
//|---@return Object object
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.object.invalid
//|function mux_world.object(dbref) end
// lua-types-end

// lua-types-begin mux 00146
//|---Sends valid UTF-8 text to an object.
//|---@param object DbRef|Object
//|---@param message string
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.connection.invalid
//|---@see mux.error.codes.object.invalid
//|function mux_world.pemit(object, message) end
// lua-types-end

// lua-types-begin mux 00147
//|---Teleports a thing or player through the native movement path.
//|---@param options TeleportOptions Teleport fields; unknown fields are rejected.
//|---
//|---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
//|---@see mux.error.codes.unavailable.checking
//|---@see mux.error.codes.arg.invalid
//|---@see mux.error.codes.object.invalid
//|---@see mux.error.codes.object.unavailable
//|function mux_world.teleport_object(options) end
// lua-types-end
