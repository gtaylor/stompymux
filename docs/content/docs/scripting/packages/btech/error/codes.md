---
title: codes
type: docs
toc_hide: false
---

Provides checked error-code symbols for BattleTech-specific failures.

## Constants

| Symbol | Code |
| --- | --- |
| `btech.error.codes.part.not_found` | `btech.part.not_found` |
| `btech.error.codes.part.ambiguous` | `btech.part.ambiguous` |
| `btech.error.codes.part.wrong_kind` | `btech.part.wrong_kind` |
| `btech.error.codes.template.not_found` | `btech.template.not_found` |
| `btech.error.codes.template.invalid` | `btech.template.invalid` |
| `btech.error.codes.operation.failed` | `btech.operation.failed` |

Errors with the `btech.operation.failed` code include a stable,
machine-readable reason string in `error.detail.reason`.

## See Also

- [`btech`](../../)
- [`btech.error`](../)
- [`mux.error.codes`](../../../mux/error/codes/)
