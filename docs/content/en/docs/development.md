---
title: Development workflows
description: Walkthroughs for typical StompyMUX game development workflows
type: docs
weight: 15
---

The sources are split across two different domains:

1. The `src/` directory contains the C sources for the game server.
1. The `game/lua/` directory contains the Lua sources for most of the in-game commands and logic. You should be able to focus most of your efforts here.

## C/Game server development workflow

To make changes to the game server sources, a typical development loop looks like this:

1. Make your changes
1. Run `just fmt-c` to format the sources
1. Run `just check-and-run` to run the full suite of checks and launch the server if successful
1. To short-circuit the checks and just build and launch, run `just build-and-run`
1. You'll need to CTRL+C the server and start over on the first step to make additional game server changes. Hot reloading the game server is not supported

### BTech persistence migrations

BTech SQLite schema migrations are checked in under `game/data/migrations/`.
Stop `stompymux`, back up `game/data/stompymux.db`, and apply the required SQL
script with `sqlite3` before starting a build with a newer schema. Migrations
are deliberately not applied by the running server.

## Lua development workflow

The Lua sources in `game/lua` are where most of the player-visible logic lives. These are typically developed like this:

1. Launch the server via `just build-and-run`
1. Make changes to Lua source(s)
1. Run `just fmt-lua` to format the sources
1. Using a Wizard character in-game, type `@lua/reload`
1. If errors are encountered when reloading or starting the game, you'll see those emitted to the server logs and to the Wizard doing the reload

More information about Lua scripting may be found in the [Scripting](./scripting/_index.md) section of the documentation.

## Documentation development workflow

We use the [Hugo](https://gohugo.io/) static site generator for our documentation.
See installation instructions [here](https://gohugo.io/installation/) if you'd like to be able to build docs locally, then:

1. Make your docs changes under `docs/content/en`
1. Run the local devserver with `just docsite-serve`
1. Point your browser at [http://localhost:1313/](http://localhost:1313/) to see the  changes
