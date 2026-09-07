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
the implemented non-BattleTech `mux` Lua binding surface. Integration suites should
restore every changed object or attribute in teardown hooks.

Run all suites as a Wizard with `@lua/test [filter]`. The optional filter is
matched as a case-sensitive literal substring of `module_path:test_name`. Use `@lua/test/unit` or
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
`error_matches(function, pattern)`, `raises`, `raises_code`, `no_error` and
`is_error`. Failed assertions report expected and
actual values separately from ordinary Lua runtime errors.


A separate VM is created for each run. It does not replace active game modules,
run startup hooks or change pending schedules. Both test directories share live
world services. Valid mutations survive assertions and runtime errors and are
saved after each module initialization, hook and test before staged output is
sent. Validation or database failure rolls back that invocation and reports an
error; applicable teardown and subsequent tests still run.

Reports go only to the invoking session; explicit script messages retain their
normal recipients. Failure and verbose pass details are limited to 64 entries
each, with omission notices and complete totals. Instruction, memory, state and
output limits still apply. `@lua/check` validates declarations without executing
test bodies or hooks. Files are read afresh for each check or test invocation.
