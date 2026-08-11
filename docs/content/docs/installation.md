---
title: Installation
description: How to install StompyMUX
type: docs
weight: 10
---

## Clone the sources

If you don't already have a copy of the sources, clone the repo:

```shell
git clone git@github.com:gtaylor/stompymux.git
```

Make sure to `cd` into the repo root and fetch the third party libraries that
we've vendored:

```
cd stompymux
git submodule update --init --recursive
```

## Install dependencies

Before compiling the sources and running your own game, you'll need to ensure that the
following dependencies are present:

* SQLite development headers
* CMake 4.3 or higher
* Clang 22 or higher
* [Just](https://github.com/casey/just) 1.56 or higher
* If you'd like to make documentation contributes, [install Hugo](https://gohugo.io/installation/)

## Building and running

Use the included `just` task runner:

```
just build-and-run
```

If `game/data/stompymux.db` does not exist, the first run creates it before
opening the game port. The console prints one-time generated passwords for the
two administrator characters. Save them immediately; only password hashes are
stored in the database.

## Connecting

Use your preferred MUD client (or `telnet`) to connec to `localhost` port `5555`.
There are two administrator characters, `#1` (`GOD`) and `#2` (`Wizard`). Use
the distinct passwords printed during first startup and change them with the
`@newpassword` command.

## Next steps

You're ready to start developing your game!
The following articles are good reads to get started:

1. [Development Workflows](./development.md)
1. [Configuration](./configuration/_index.md)
