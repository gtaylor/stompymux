local t = require("testing")

return t.suite("assertion", {
  tests = {
    t.test("fails", function(_, expect)
      expect.equal("actual", "expected")
    end),
  },
})
