---
title: mux.error package
linkTitle: mux.error
type: docs
weight: -50
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
| [`new`](new/) | Creates a structured error value. |
| [`raise`](raise/) | Raises a structured error. |
| [`wrap`](wrap/) | Adds a structured error around a cause. |
| [`check`](check/) | Returns a truthy value or raises a supplied error. |
| [`pcall`](pcall/) | Calls a function and returns normalized failures. |
| [`is`](is/) | Tests an error value against a code or code namespace. |
| [`code_tree`](code-tree/) | Returns the checked tree for a native code namespace root. |

### Defining codes

| Function | Description |
| --- | --- |
| [`namespace`](namespace/) | Builds checked symbols for author-defined error codes. |

## Types

| Type | Description |
| --- | --- |
| [`Error`](type-error/) | A structured Lua error with code, message, and optional context. |

## Code constants

| Constants | Description |
| --- | --- |
| [`mux.error.codes`](codes/) | Checked native error-code symbols and their namespace nodes. |
