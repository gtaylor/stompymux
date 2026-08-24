---
title: mux.config package
linkTitle: config
type: docs
weight: 15
sidebar_root_for: self
no_list: true
---

`mux.config` provides read-only access to the server's current scalar
configuration. Configuration directives that modify collections, aliases,
permissions, flags, sites, or bitmask members are intentionally unavailable.

## Functions

| Function | Description |
| --- | --- |
| [`mux.config.get`](get/) | Returns the live value of a scalar configuration directive. |

The package is available in running callbacks and during `@lua/check`.
