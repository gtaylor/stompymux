+++
title = "@lua/test"
description = "Run fully mutable Lua test suites"
keywords = ["@lua/test", "lua testing", "testing"]
article_tags = ["lua_switches"]
weight = 60
wizard_only = true
+++

# @lua/test

> **Warning:** Lua tests are fully mutable. They run against whatever database
> the server has loaded and all writes land for real. Run tests against a
> scratch database, never production.

Put suites below `game/lua/tests/unit/` or `game/lua/tests/integration/`. The
directories are an authoring and command-filter convention only: both receive
the full `mux` and `btech` Lua binding surface. Integration suites should
restore every changed object or attribute in teardown hooks.

Run all suites as a Wizard with `@lua/test [filter]`. The optional filter is
matched against `module_path:test_name`. Use `@lua/test/unit` or
`@lua/test/integration` to limit discovery to one directory; append `/verbose`
to list passing tests.

Each suite returns `testing.suite(...)` from the shared `testing` package:

```lua
local t = require("testing")

return t.suite("cargo transfers", {
  before_all = function(ctx) end,
  after_all = function(ctx) end,
  before_each = function(ctx) end,
  after_each = function(ctx) end,
  tests = {
    t.test("moves between rooms", function(ctx, expect)
      expect.equal(actual, expected)
    end),
  },
})
```

`ctx` is a new table for each suite. Values added in `before_each` are visible
to that test and its `after_each`. Hooks run in this order: `before_all`, then
for each test `before_each`, test, `after_each`, followed by `after_all`.
`after_each` and `after_all` still run after failures. A failed `before_all`
marks its suite's tests as errored without running them, then `after_all` runs.

The `expect` table provides `equal`, `not_equal`, `truthy`, `falsy`, `is_nil`,
`contains`, `near(actual, expected, tolerance)`, and
`error_matches(function, pattern)`. Failed assertions report expected and
actual values separately from ordinary Lua runtime errors.
