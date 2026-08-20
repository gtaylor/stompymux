---
title: mux.text
linkTitle: mux.text
type: docs
weight: -30
no_list: true
sidebar_root_for: self
---

`mux.text` provides validation, styling, measurement, and truncation helpers for
styled text.

## Functions

| Function | Description |
| --- | --- |
| [`is_printable_ascii`](is-printable-ascii/) | Tests whether a string contains only printable ASCII bytes. |
| [`markup`](markup/) | Validates styled-text markup. |
| [`strip_style`](strip-style/) | Removes markup and ANSI styling. |
| [`style`](style/) | Applies styles described by an options table. |
| [`truncate`](truncate/) | Truncates styled text to a visible width. |
| [`width`](width/) | Returns the visible byte width of styled text. |

See [Styled Text](../../../../concepts/styled-text/) for an overview of the
markup format and supported presentation features.
