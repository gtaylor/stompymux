---
title: Installation
description: How to install StompyMUX
type: docs
weight: 10
---
## Requirements

- **StompyMUX only builds on Linux**. It has been primarily developed on Ubuntu 24.04 but most quasi-recent distributions should fit the bill.

The recommended development environment is the repository's devcontainer. It
contains the complete, tested toolchain and avoids downloading compiler packages
during ordinary builds. Open the repository in a devcontainer-capable editor or
Codespace and rebuild the container when prompted.

## Clone the sources

Fork and clone the repo and then `cd` into it:

```shell
## < Fork here and substitute your fork URLs in the text below >
git clone git@github.com:gtaylor/stompymux.git
cd stompymux
```

## Install dependencies

Before compiling the sources and running your own game, you'll need to ensure that the
following dependencies are present:

- SQLite development headers
- CMake 4.3 or higher
- Clang 22 or higher -- see [apt.llvm.org](http://apt.llvm.org) for an APT repo with recent Clang versions for Debiain/Ubuntu
- [Just](https://github.com/casey/just) 1.56 or higher
- If you'd like to make documentation contributes, [install Hugo](https://gohugo.io/installation/)

## Building and running

Use the included `just` task runner:

```shell
just update-submodules
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
2. [Configuration](./configuration/_index.md)
