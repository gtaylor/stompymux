# Lua tests

Put Lua test suites here. Use `unit/` for tests that exercise Lua helpers and
`integration/` for tests that deliberately use the full MUX binding surface.
Both directories run with the same bindings and can mutate the loaded game
database. Keep integration-test cleanup in `after_each` or `after_all` hooks.

Suites are discovered recursively in lexical relative-path order. Shared Lua
helpers belong in `../packages` and are imported with dotted `require` names.
