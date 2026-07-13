# AGENTS instructions

## Repository layout

* `game.run`: Files needed to run the game server.
* `game.run/text`: Sources for the `help` and `wizhelp` commands, along with other larger blocks of static text (MOTD, new user notices, etc)
* `docs`: Docs for the game server and its sources.
* `src`: Location of all C sources for the game and its supporting utilities.
* `src/mux`: Base MUX game server sources.
* `src/btech`: Battletech extensions that layer on top of the base MUX game server.
* `src/mkindx`: Sources for the `mkindx` bin for indexing help files.
* `tests`: Integration tests.

## Development rules

* Name functions in the format of `[module]_[verb]`.
* Name structs and types in the form of `[module]_[name]_[c-type-letter].
* Two space indents for C sources.
* Avoid the use of preprocessor macros when possible.

## Development workflows

* We use the `just` command runner
* When making changes, run `just list-changes`, `just build`, `just test`, and then `just install` to validate end to end.