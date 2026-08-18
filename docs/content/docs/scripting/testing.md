---
title: Lua testing
description: Write and run isolated, fully mutable Lua test suites
type: docs
weight: 35
---

## Important safety warning

Lua tests are fully mutable and run against the database currently loaded by
the server. They do not use a transaction or a write barrier. Use a scratch
database, never production, and restore all object and attribute changes in
teardown hooks.

## Suites and commands

Place suites under `game/lua/tests/unit/` or `game/lua/tests/integration/`.
This is a convention and a discovery filter; both have the complete `mux` and
`btech` API surface. Run suites as a Wizard:

```text
@lua/test [filter]
@lua/test/unit [filter]
@lua/test/integration [filter]
```

The optional filter matches `module_path:test_name`. Passing tests are silent
unless `/verbose` is supplied. Failures include a traceback, and the summary
distinguishes assertions, runtime errors, and hook errors.

## Writing a suite

```lua
local t = require("testing")

return t.suite("cargo transfers", {
  before_each = function(ctx)
    ctx.original_location = 42
  end,
  after_each = function(ctx)
    -- Restore changes made by the test.
  end,
  tests = {
    t.test("moves between rooms", function(ctx, expect)
      expect.equal(actual, expected)
    end),
  },
})
```

The hooks are `before_all`, `before_each`, `after_each`, and `after_all`.
Each suite receives a fresh `ctx` table; data placed there by `before_each` is
available to its test and `after_each`. Cleanup hooks run even after a test or
earlier hook fails. A failed `before_all` errors every test in that suite and
still permits `after_all` to run.

`expect` provides `equal`, `not_equal`, `truthy`, `falsy`, `is_nil`,
`contains`, `near`, `error_matches`, `raises`, `raises_code`, `no_error`, and
`is_error`. `raises` returns the structured error so tests can inspect its
code and detail. Assertion errors keep expected and actual values structured,
so the runner can report them differently from Lua runtime errors. See the
[`mux.error` reference](packages/mux-error/) for error fields and matching.

## Testing error codes

The `testing` package exposes `testing.error.codes.assertion` and
`testing.error.codes.runtime`, with string values `testing.assertion` and
`testing.runtime`. They are checked symbols for assertions and normalized
runtime failures respectively. The raising and inspection functions remain on
`mux.error`.
