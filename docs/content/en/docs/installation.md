---
title: Installation
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

* CMake 4.3 or higher
* Clang 20 or higher
* [Just](https://github.com/casey/just) 1.56 or higher

## Building and running

Use the included `just` task runner:

```
just install
just run
```

If all goes well, you'll see a StompyMUX log stream.

## Connecting

Use your preferred MUD client (or `telnet`) to connec to `localhost` port `5555`.
There are two Wizard (admin) characters, `#1` (God) and `#2` (Wizard). They both
have the same default password of `btmuxr0x`. Change these as soon as possible
with the `@newpassword` command.

## Compile options

There are a number of compile-time options exposed in `CMakeLists.txt` in the repo root.
See these for a canonical list. Due to how much history has built up over time, your
best bet is to review the sources to see what these do.
