# AGENTS instructions

## Repository layout

* `game`: Files needed to run the game server.
* `game/help`: Markdown+TOML-frontmatter articles served by the `help` command.
* `game/text`: Other larger blocks of static text (MOTD, new user notices, etc)
* `docs`: Docs for the game server and its sources.
* `src`: Location of all C sources for the game and its supporting utilities.
* `src/mux`: Base MUX game server sources.
* `src/mux/help`: Help article indexing, rendering, and command handlers.
* `src/btech`: Battletech extensions that layer on top of the base MUX game server.
* `tests`: Integration tests.

## C coding rules

* Make use of C23 features where it simplifies things.
* Two space indents for C sources.
* Avoid the use of preprocessor macros when possible.
* Use bool types instead of int for boolean logic.
* uSE `nullptr` instead of NULL or other workarounds.
* Prefer multiple bools instead of bitmasks where reasonable.
* Use enums instead of multiple #define or constexpr statements.
* Obey the C code naming conventions below.

## C code naming conventions

When writing C code, use the following naming conventions:

* All macros and constants should be in caps: `THIS_IS_A_MACRO`, `ANOTHER_EXAMPLE_MACRO`
* Struct names and typedefs should be in PascalCase: `FileDescription`, `MechObject`
* Functions that operate on structs should use classic C style naming in snake_case: `file_descriptor_write`, `mech_object_destroy`

## Development workflows

* We use the `just` command runner
* When making changes, run `just agent-checks` to validate end to end.
* Make sure that updates to behaviors are reflected in `game/help/`, and `docs/`.
* Check the various `game/*.conf` and `game/*.config` files when making changes to mudconfs, configs, and settings.
* If making DB schema changes, offer to update the game's database at `game/data/stompymux.db.sqlite`. If a `stompymux` process is running, direct me to shutdown the game before making changes or instability could occur.

## Available clang tools

I've installed `clang-20`. There are a number of other tools installed, but they all have the version `-20` suffix:

* clang-tidy-20
* clang-format-20
* clang-apply-replacements-20
* clang-query-20

See `ls /usr/bin/clang*` for a full list.