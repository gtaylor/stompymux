---
title: Name generation
weight: 33
description: Seeded names for mercenary outfits, the people in them and their callsigns
---

`namegen` produces names for mercenary outfits ("Ragged Kraken Rangers"),
people ("Milena Tomic") and pilot callsigns ("Switchback"). The generator is
the `stompymux-namegen` library in `crates/namegen`. It does not depend on the
server, and the `namegen` command-line tool is a thin wrapper around it.

## Quick start

```sh
just namegen outfit -n 20 --flavor pirate
just namegen roster -n 6 --culture slavic --seed 4
just namegen callsign -n 10
just namegen outfit --seed 6 --json
```

`just namegen` is `cargo run -p stompymux-namegen --bin namegen --`. `-n`
prints several names; with `--seed`, the first is the name that seed gives on
its own and the rest derive their seeds from it. `--json` adds the resolved
spec so a name can be reproduced later. `just namegen lists` summarizes the
word lists in use.

## Specs and determinism

Every name is a pure function of a spec with three optional fields:

| Field | Meaning | When omitted |
|---|---|---|
| `seed` | The random seed | Taken from the clock |
| `culture` | Naming tradition for people and possessive outfit names: `anglo`, `hispanic`, `east_asian`, `slavic`, `germanic`, `arabic`, `south_asian`, `west_african` | Drawn from the seed |
| `flavor` | Outfit style: `house` (numbered regiments, formal unit types), `periphery` (frontier militia), `pirate` (menacing, commander's name up front) | Drawn from the seed |

Culture and flavor come from their own random streams, so fixing one by hand
never changes the other. Each kind of name also draws from its own stream, so
an outfit name, its commander (`person`) and its roster (`roster`) can all share
one seed without overlapping. Rosters avoid repeated family names and callsign
batches avoid repeats where the lists allow it.

The generator is `crates/seeded-rng`, a pinned SplitMix64 also used by map
generation. Its algorithm never changes, so a seed stored with a company
reproduces the same names in every later release. The library's snapshot test
pins a handful of seeds for the same reason: changing the word lists or
patterns changes names for existing seeds, so do it deliberately.

## Word lists

Outfit names come from a small grammar in `crates/namegen/data/outfit.toml`.
A pattern is literal text with `{slot}` placeholders and a selection weight:

```toml
[[patterns]]
template = "{ordinal} {place} {suffix}"
weight = 16
flavors = ["house", "periphery"]
```

Slots backed by word lists are `adjective`, `noun`, `suffix`, `color` and
`place`. Two slots are derived: `ordinal` gives a regiment number such as
"21st" and `possessive` gives a family name from the chosen culture such as
"Reyes'". A pattern without `flavors` is open to every flavor. Each word list
has a `common` core plus optional `house`, `periphery` and `pirate` additions,
and a `blocked` list names fragments that must never appear, so accidental
collisions with well-known fictional units are rerolled.

Callsigns live in `callsigns.toml` and people's names in one file per culture
under `people/`. Loading validates every list: entries must be unique, free of
surrounding whitespace and present for every pattern a flavor can reach, so
generation itself can never fail.

The embedded lists can be overridden from a directory with
`--lists DIR` (or `Lists::from_dir` in code). Only the files present in the
directory are replaced, matched by relative path such as `people/anglo.toml`.
The server reads overrides from `data/names` under the game directory when
that directory exists. Keep every entry original; do not add named units from
published fiction.

## From Lua

Game modules reach the generator through the `mux.names` package, which is
pure and stays available during `@lua/check`. Cultures and flavors are typed
constants rather than strings:

```lua
local names = mux.names
local spec = { seed = 42, culture = names.cultures.SLAVIC, flavor = names.flavors.PIRATE }
local outfit = names.outfit(spec)        -- outfit.text, outfit.pattern, outfit.seed, ...
local commander = names.person(spec)     -- commander.full, .given, .family, .culture
local pilots = names.roster(spec, 8)     -- distinct family names
local handles = names.callsigns(spec, 8) -- distinct callsigns
```

Every field of the spec is optional. A result carries the seed it used, so a
module can store that integer and reproduce the same names later. Seeds are
limited to 2^53 - 1 so Lua numbers hold them exactly. The full reference is
under [`mux.names`](../../scripting/packages/mux/names/).
