local t = require("testing")

return t.suite("before each", {
  before_each = function()
    error("before_each exploded")
  end,
  tests = {
    t.test("does not run", function()
      error("test should not run")
    end),
  },
})
