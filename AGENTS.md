# AGENTS instructions

## Repository layout

- `game`: Files needed to run the game server.
- `game/help`: Markdown+TOML-frontmatter articles served by the `help` command.
- `game/text`: Other larger blocks of static text (MOTD, new user notices, etc)
- `docs`: Docs for the game server and its sources.
- `src`: Location of all C sources for the game and its supporting utilities.
- `src/mux`: Base MUX game server sources.
- `src/mux/help`: Help article indexing, rendering, and command handlers.
- `src/btech`: Battletech extensions that layer on top of the base MUX game server.
- `tests`: Unit and integration tests.

## C coding rules

- Make use of C23 features where it simplifies things.
- Two space indents for C sources.
- Avoid the use of preprocessor macros when possible.
- Use bool types instead of int for boolean logic.
- Use `nullptr` instead of NULL or other workarounds.
- Prefer multiple bools instead of bitmasks where reasonable.
- Use enums instead of multiple #define or constexpr statements.
- Obey the C code naming conventions below.
- Avoid source files longer than 800 lines long.
- Avoid transitive includes and clean them up where possible.
- Timestamps stored in SQLite should always be in UTC.

## C code naming conventions

When writing C code, use the following naming conventions:

- All macros and constants should be in caps: `THIS_IS_A_MACRO`, `ANOTHER_EXAMPLE_MACRO`
- Struct names and typedefs should be in PascalCase: `FileDescription`, `MechObject`
- Functions that operate on structs should use classic C style naming in snake_case: `file_descriptor_write`, `mech_object_destroy`

## Testing Practices

- Unit and integration tests must not interact with the production `game/` directory. Copy fixtures to tests/fixtures/game and tests/fixtures/{integration|unit} instead.
- Unit tests may be ran outside of a stompymux or stompymux-like process. These are in `tests/unit/``
- Integration/E2E tests require something resembling part of the game server. These are in `tests/integration/`
- The `tests/fixtures` directory contains a freestanding collection of files required for unit and integration tests.
- The `tests/fixtures/game` directory is a minimal game directory for the maximal cases.
- Avoid hardcoded sleeps where possible. Prefer watches and other techniques to keep our test suite time low.
- `tests` directory structure doesn't have to exactly match the source structure, but keep test suites grouped into topical subdirectories.

## Development workflows

- We use the `just` command runner
- When making changes, run `just agent-checks` to validate end to end.
- Make sure that updates to behaviors are reflected in `game/help/`, and `docs/`.
- Check the various `game/*.conf` and `game/*.config` files when making changes to mudconfs, configs, and settings.
- If making DB schema changes, offer to update the game's database at `game/data/stompymux.db`. If a `stompymux` process is running, direct me to shutdown the game before making changes or instability could occur.

## Available clang tools

I've installed `clang-22`. There are a number of other tools installed, but they all have the version `-22` suffix:

- clang-tidy-22
- clang-format-22
- clang-apply-replacements-22
- clang-query-22

See `ls /usr/bin/clang*` for a full list.