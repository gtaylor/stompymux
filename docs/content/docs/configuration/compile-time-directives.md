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
require a special Cargo build. `Cargo.toml` defines two opt-in features for
developer tools:

| Feature | Binary | Purpose |
| --- | --- | --- |
| `lua-type-updater` | `lua-type-updater` | Regenerate LuaLS type declarations |
| `lua-doc-updater` | `lua-doc-updater` | Regenerate Lua API reference pages |

Run them through `just update-lua-types` and `just update-lua-docs`, or with
`cargo run --features <feature> --bin <binary> -- --write`.
