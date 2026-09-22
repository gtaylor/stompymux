---
title: "Contributing"
weight: 70
description: How to contribute to StompyMUX
---

Review the license and contribution terms of the repository you are targeting
before submitting changes. The [history](/docs/history/) page preserves the
predecessor project's credits and copyright notices.

To contribute changes back to StompyMUX, file a pull request with a descriptive title and body.
Since StompyMUX is a hobby project, we are unable to make guarantees about timeliness and we may reject changes that don't align with the project's direction.
Feel free to discuss ideas in our [Discord server](https://discord.gg/TJzQByY2nC) before starting implementation in order to reduce the likelihood of a contributing being rejected.

The server implementation is in Rust under `src/`; game behavior and examples
also live in `game/lua/`. Format Rust changes with `cargo fmt`, run
`cargo test`, and build the documentation site for doc changes. See
[Development workflows](/docs/development/) for the commands and test layout.
