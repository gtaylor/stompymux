---
title: btech.error package
linkTitle: btech.error
type: docs
weight: -50
sidebar_root_for: self
no_list: true
---

`btech.error` contains checked symbols for BattleTech-specific failures. The
functions for creating, raising, wrapping, and inspecting errors live on
[`mux.error`](../../mux/error/).

BattleTech code should use `mux.error.codes` for generic failures such as
invalid arguments. Use `btech.error.codes` only for BattleTech-subsystem
failures.

## Code constants

| Constants | Description |
| --- | --- |
| [`btech.error.codes`](codes/) | Checked BattleTech error-code symbols. |
