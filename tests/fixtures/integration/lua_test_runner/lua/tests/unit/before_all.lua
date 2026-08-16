local t = require("testing")

return t.suite("before all", {
  before_all = function()
    error("before_all exploded")
  end,
  tests = {
    t.test("first", function() end),
    t.test("second", function() end),
  },
})
