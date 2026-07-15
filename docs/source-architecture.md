# Source architecture

The MUX server is organized by responsibility beneath `src/mux`.

- `server` contains platform definitions, configuration parsing, server state,
  lifecycle, logging, timers, signals, and file caches.
- `support` contains reusable containers, buffer helpers, and string utilities.
- `database` owns game objects, attributes, flags, powers, locks, and virtual
  attributes.
- `world` owns player, object, matching, movement, and presentation behavior.
- `commands` owns command dispatch, queues, evaluation, macros, and help.
- `communication` owns channels, communications attributes, and speech.
- `network` owns client descriptors, Telnet, sockets, and event scheduling.
- `persistence` owns SQLite-backed MUX data.
- `lua` owns the Lua runtime integration.

Project code includes MUX interfaces through paths rooted at `mux/`; it does
not depend on the include-directory search order. Types exposed by MUX use
descriptive PascalCase names. Functions use snake_case, and implementation
details remain private to their owning module unless another translation unit
needs them.
