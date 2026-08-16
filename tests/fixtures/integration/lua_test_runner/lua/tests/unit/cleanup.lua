local t = require("testing")

return t.suite("cleanup", {
  after_each = function()
    error("after_each cleanup ran")
  end,
  after_all = function()
    error("after_all cleanup ran")
  end,
  tests = {
    t.test("fails before cleanup", function(_, expect)
      expect.equal(false, true)
    end),
  },
})
