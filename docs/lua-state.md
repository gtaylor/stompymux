# Persistent object state

The built-in implementation lives in `src/lua/packages/world/state.rs`; validation
and native commands share `src/state`. `game/lua/packages/access_policy.lua` is
editable game logic and is not embedded or rewritten by the state subsystem.

```lua
local state = mux.world.object(ctx.enactor):state("quest.progress")
state:set_many({ visits = 1, finished = false, token = string.char(0, 255) })
local previous = state:get("missing", { fallback = true })
assert(state:has("finished")) -- present false is distinct from missing
local selected = state:get_many({ "visits", "missing" })
for _, entry in ipairs(state:entries()) do
  -- entry.key and entry.value; enumeration is bytewise sorted
end
state:delete("token") -- true if removed, false if absent
state:set("finished", nil) -- also removes
```

`get`, `has` and `get_many` can read state while modules load. `set`, `delete`,
`set_many`, `keys` and `entries` require a callback transaction. The namespace
handle is immutable and validates its object's runtime incarnation on every
operation, including `tostring`. New provisional objects invalidated by rollback
cannot make old handles valid again by reusing the same dbref.

Only strings, booleans and finite numbers are values. LuaJIT integral numbers
within signed 64-bit range are stored as integers. SQLite preserves integer
precision even when it exceeds LuaJIT's exact numeric range; avoid reading and
writing such values through arithmetic when exact precision is required. Strings
are byte sequences, not necessarily UTF-8. No tables or implicit serialization
are supported. An absent `get` returns the exact supplied default, with its
identity and type intact.

`set_many` is atomic even if its caller catches an error. It validates all keys,
values and final namespace/key/value byte accounting before changing the world.
Use `delete` or `set(key, nil)` for removals. Callback failure discards mutations
and staged messages; successful callbacks remain staged until the surrounding
server transaction is durable. Malformed lock results also roll back.

Lock callbacks may return a boolean or a table containing a boolean `passes`
and optional string `enactor_message` and `other_message` fields. Unknown fields
and invalid types fail closed. Messages must be shorter than 8192 bytes and valid
UTF-8 for the game text renderer; invalid UTF-8 message strings fail closed. Valid text still passes through the
shared renderer, which handles terminal control sequences. `mux.world.lock_passes` retains its boolean interface.
