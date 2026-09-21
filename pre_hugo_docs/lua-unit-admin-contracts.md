# Lua unit administration contracts

The supported live BattleMech, ground-vehicle, and VTOL adapters use the C public names and typed constant catalogs. Mutations return zero Lua values. The binding validates the unit before later arguments, accepts C numeric ranges, and keeps identity strings, pilot assignment, protection, movement, cooling, sensor, radio, cargo, and configured technology state in the ordinary persistent unit model.

`set_armor` reads patch fields through ordinary Lua indexing, so a metatable `__index` can supply `armor`, `internal`, or `rear_armor`. Values are integral 0 through 255 and a patch must provide at least one field. It changes both original and current material, matching the native administrative operation.

Rust currently represents Mechs, ground vehicles, and VTOLs as distinct validated domain types. `set_unit_type` is exact when the requested type matches that represented chassis. Cross-family conversion and the remaining naval, aerospace, personal, and battlesuit types require their missing gameplay domains. Technology codes backed by existing configured features are supported; infantry-only technology requires the battlesuit domain, and codes for gameplay systems absent from Rust remain input-level prerequisites.

`set_cargo_capacity` validates both C arguments. The existing carrier load model stores capacity; the reference's independent maximum carried-unit tonnage still needs a dedicated domain field before that portion can be verified. Tonnage above Rust's current `u16` construction range likewise needs a wider construction representation even though C accepts values through `INT_MAX / 1024`.
