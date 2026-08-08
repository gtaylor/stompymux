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

If all goes well, you'll see a StompyMUX log stream.

## Connecting

Use your preferred MUD client (or `telnet`) to connec to `localhost` port `5555`.
There are two Wizard (admin) characters, `#1` (God) and `#2` (Wizard). They both
have the same default password of `btmuxr0x`. Change these as soon as possible
with the `@newpassword` command.

## Next steps

You're ready to start developing your game!
The following articles are good reads to get started:

1. [Development Workflows](./development.md)
1. [Configuration](./configuration/_index.md)