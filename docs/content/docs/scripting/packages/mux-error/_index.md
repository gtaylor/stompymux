---
title: mux.error package
linkTitle: mux.error
type: docs
weight: 15
sidebar_root_for: self
no_list: true
---

`mux.error` is the built-in structured-error API. It provides stable error
codes, readable messages, optional details and causes, and helpers for
raising, inspecting, and preserving errors across Lua call boundaries.

Use a raised error for a contract violation or a failure the current callback
cannot recover from. For an expected, recoverable failure, return `nil, err`
instead, so the caller can decide whether and how to handle the `Error`.

## Functions

### Creating and handling errors

| Function | Description |
| --- | --- |
| [`mux.error.new`](new/) | Creates a structured error value. |
| [`mux.error.raise`](raise/) | Raises a structured error. |
| [`mux.error.wrap`](wrap/) | Adds a structured error around a cause. |
| [`mux.error.check`](check/) | Returns a truthy value or raises a supplied error. |
| [`mux.error.pcall`](pcall/) | Calls a function and returns normalized failures. |
| [`mux.error.is`](is/) | Tests an error value against a code or code namespace. |
| [`mux.error.code_tree`](code-tree/) | Returns the checked tree for a native code namespace root. |

### Defining codes

| Function | Description |
| --- | --- |
| [`mux.error.namespace`](namespace/) | Builds checked symbols for author-defined error codes. |

## Types

| Type | Description |
| --- | --- |
| [`Error`](type-error/) | A structured Lua error with code, message, and optional context. |

## Code constants

| Constants | Description |
| --- | --- |
| [`mux.error.codes`](codes/) | Checked native error-code symbols and their namespace nodes. |
