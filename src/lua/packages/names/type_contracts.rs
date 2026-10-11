//! LuaLS contract blocks for the mux.names surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00174
//|---A typed naming-culture constant from [`mux.names.cultures`](lua://mux.names.cultures).
//|---Its string form is the snake_case name such as `slavic`, and equality compares
//|---the culture.
//|---@class NameCulture
// lua-types-end

// lua-types-begin mux 00175
//|---A typed outfit-flavor constant from [`mux.names.flavors`](lua://mux.names.flavors).
//|---Its string form is the snake_case name such as `pirate`, and equality compares
//|---the flavor.
//|---@class NameFlavor
// lua-types-end

// lua-types-begin mux 00176
//|---Immutable lookup namespace for naming-culture constants.
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid)
//|---for unknown or non-string keys and attempted mutation.
//|---@class NameCultureNamespace
//|---@field ANGLO NameCulture English-speaking naming tradition.
//|---@field HISPANIC NameCulture Spanish and Latin American naming tradition.
//|---@field EAST_ASIAN NameCulture Japanese, Chinese, Korean and Vietnamese naming tradition.
//|---@field SLAVIC NameCulture Russian, Polish and South Slavic naming tradition.
//|---@field GERMANIC NameCulture German and Scandinavian naming tradition.
//|---@field ARABIC NameCulture Arabic and Levantine naming tradition.
//|---@field SOUTH_ASIAN NameCulture Indian, Pakistani and Bangladeshi naming tradition.
//|---@field WEST_AFRICAN NameCulture Ghanaian, Nigerian and Sahelian naming tradition.
//|---@see mux.error.codes.arg.invalid
// lua-types-end

// lua-types-begin mux 00177
//|---Immutable lookup namespace for outfit-flavor constants.
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid)
//|---for unknown or non-string keys and attempted mutation.
//|---@class NameFlavorNamespace
//|---@field HOUSE NameFlavor Former house regulars: numbered regiments, places, formal unit types.
//|---@field PERIPHERY NameFlavor Frontier militia turned professional: homespun, rugged, place-bound.
//|---@field PIRATE NameFlavor Raiders and bandits: menacing, boastful, a commander's name up front.
//|---@see mux.error.codes.arg.invalid
// lua-types-end

// lua-types-begin mux 00178
//|---What to generate. Every field is optional. The same seed, culture and flavor always
//|---produce the same names; an omitted seed comes from the clock, and an omitted culture
//|---or flavor is drawn from the seed on its own stream, so fixing one never changes the
//|---other.
//|---@class (exact) NameSpec
//|---@field seed? integer Random seed from 0 to 2^53 - 1.
//|---@field culture? NameCulture Naming tradition for people and possessive outfit names.
//|---@field flavor? NameFlavor Outfit naming style.
// lua-types-end

// lua-types-begin mux 00179
//|---A generated outfit name and the resolved spec that reproduces it.
//|---@class OutfitName
//|---@field text string The name, such as `Ragged Kraken Rangers`.
//|---@field pattern string The grammar template the name was built from, such as `{adjective} {noun} {suffix}`.
//|---@field seed integer The seed used.
//|---@field culture NameCulture The culture used.
//|---@field flavor NameFlavor The flavor used.
// lua-types-end

// lua-types-begin mux 00180
//|---A generated person's name.
//|---@class PersonName
//|---@field given string Given name.
//|---@field family string Family name.
//|---@field full string Given name then family name.
//|---@field culture NameCulture The culture the name came from.
// lua-types-end

// lua-types-begin mux 00181
//|---Seeded mercenary outfit, person and callsign names. Pure and available during
//|---`@lua/check`. Word lists come from `data/names` under the game directory when
//|---that directory exists, otherwise from the lists built into the server.
//|---@class MuxNamesPackage
//|---@field cultures NameCultureNamespace Immutable namespace of naming-culture constants.
//|---@field flavors NameFlavorNamespace Immutable namespace of outfit-flavor constants.
//|local mux_names = {}
// lua-types-end

// lua-types-begin mux 00182
//|---Generates a mercenary outfit name such as `35th Harrow Heavy Horse`.
//|---@param spec? NameSpec
//|---@return OutfitName outfit
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for a
//|---malformed spec.
//|---@see mux.error.codes.arg.invalid
//|function mux_names.outfit(spec) end
// lua-types-end

// lua-types-begin mux 00183
//|---Generates one person's name in the spec's culture. For the same spec it does not
//|---overlap with [`mux.names.roster`](lua://mux.names.roster), so a commander and their
//|---roster can share a seed.
//|---@param spec? NameSpec
//|---@return PersonName person
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for a
//|---malformed spec.
//|---@see mux.error.codes.arg.invalid
//|function mux_names.person(spec) end
// lua-types-end

// lua-types-begin mux 00184
//|---Generates `count` people in one culture with distinct family names where the lists
//|---allow it.
//|---@param spec? NameSpec
//|---@param count integer From 1 to 500.
//|---@return PersonName[] roster
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for a
//|---malformed spec or count.
//|---@see mux.error.codes.arg.invalid
//|function mux_names.roster(spec, count) end
// lua-types-end

// lua-types-begin mux 00185
//|---Generates one pilot callsign such as `Switchback`.
//|---@param spec? NameSpec
//|---@return string callsign
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for a
//|---malformed spec.
//|---@see mux.error.codes.arg.invalid
//|function mux_names.callsign(spec) end
// lua-types-end

// lua-types-begin mux 00186
//|---Generates `count` distinct pilot callsigns where the list allows it.
//|---@param spec? NameSpec
//|---@param count integer From 1 to 500.
//|---@return string[] callsigns
//|---
//|---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for a
//|---malformed spec or count.
//|---@see mux.error.codes.arg.invalid
//|function mux_names.callsigns(spec, count) end
// lua-types-end

// lua-types-begin mux 00187
//|mux.names = mux_names
// lua-types-end
