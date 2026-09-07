# Lua tests

Put Lua test suites here. Use `unit/` for tests that exercise Lua helpers and
`integration/` for tests that deliberately use the full MUX binding surface.
Both directories run with the same bindings and can mutate the loaded game
database. Keep integration-test cleanup in `after_each` or `after_all` hooks.

Suites are discovered recursively in lexical relative-path order. Shared Lua
helpers belong in `../packages` and are imported with dotted `require` names.

Run `@lua/test [filter]`, selecting `/unit` or `/integration` as needed; `/verbose`
includes passing names. Filters match `module_path:test_name` literally and
case-sensitively. Files outside those directories are selected when both are
requested (including the default). Entirely filtered suites run no hooks.

Valid mutations persist even after failed tests. Each invocation validates and
saves before output; failed persistence rolls back that invocation. Test VM
globals are separate from active game code. `@lua/check` validates declarations
without running hooks or tests. See `help @lua/test` for hook ordering and counts.
