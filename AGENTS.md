# stompymux-rs AGENTS.md

stompymux-rs is a Rust rewrite of stompymux, a C-based MUD server that includes a Battletech-based realtime combat system.

## Principals

* We're pre-1.0 so you need not write migration, shim, or "legacy bridge" code because there are on the whole no existing production users of the codebase. Do not litter the project with code which "supports legacy users" or somesuch.

## Core workflow

1. Make your changes.
1. Run `cargo fmt` and `cargo test` before handing back to the human.

## Rust rules

* This code targets the 2024 Rust edition.
* Strongly prefer early returns. Avoid deep nesting.
* Crates should export their internals from their main lib.rs, and downstream users of those crates should not -- in general -- be going down into modules within the crate to access things.
* Avoid unsafe code blocks where possible.
* Leave a comment describing what each Rust file is for.
* Do not leave comments which refer to older ways of doing things.
* Document functions, types, and other variables with comments.
* Do make sure that major functions and modules have adequate Rustdoc.
* Add unit tests beside implementations (#[cfg(test)] modules) and integration tests under each crate’s tests/ directory.
* Ensure that there is a newline between Rust blocks (ex: functions, structs, enums, etc) for readability.