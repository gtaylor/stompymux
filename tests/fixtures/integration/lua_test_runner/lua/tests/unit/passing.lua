local t = require("testing")

return t.suite("passing", {
  tests = {
    t.test("works", function(_, expect)
      expect.equal(2, 2)
    end),
  },
})
