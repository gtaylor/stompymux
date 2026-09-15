# BattleTech characterization assets

`mechs/JR7-D`, `mechs/AS7-D` and `maps/test.map` were copied unchanged from the
reference checkout's `tests/fixtures/game` directory. `maps/environment.map` was
copied from `tests/fixtures/unit/map_load/environment.map`.

These are data fixtures, not translated C sources. Tests copy them into temporary
directories; they do not access either checkout's production game data and do not
require `btmux-khi` to be present.

The environment fixture's `42: 88 19` line establishes the flags/gravity/temperature
syntax. Non-square maps, malformed inputs and path-boundary fixtures are generated
explicitly by the tests.
