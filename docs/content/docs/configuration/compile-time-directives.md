---
title: Build options
linkTitle: Build options
description: Cargo build profiles and optional developer tools
weight: 30
---

StompyMUX is built with Cargo. Use the default development profile while
iterating, or build an optimized binary with the release profile:

```sh
cargo build
cargo build --release
```

Game rules and server settings are configured in `stompymux.toml`; they do not
require a special Cargo build.

The developer tools that keep generated Lua files in sync live in the
`stompymux-lua-tools` crate under `crates/lua-tools`. They read Rust sources as
text and do not link the server, so they build in seconds:

| Binary | Purpose |
| --- | --- |
| `lua-type-updater` | Regenerate LuaLS type declarations |
| `lua-doc-updater` | Regenerate Lua API reference pages |

Run them through `just update-lua-types` and `just update-lua-docs`, or with
`cargo run -p stompymux-lua-tools --bin <binary> -- --write`.
