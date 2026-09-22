---
title: "mux.check_db"
type: docs
linkTitle: "check_db"
manualLinkTitle: "check_db"
---

Checks the database for inconsistencies and repairs damage found by the
default native `@dbck` pass. Findings are written to the server log.

Raises `mux.error.codes.unavailable.checking`.

## Signature

```lua
mux.check_db()
```

## Parameters

None.

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
