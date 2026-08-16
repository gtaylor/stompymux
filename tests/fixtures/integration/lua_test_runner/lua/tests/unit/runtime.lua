local t = require("testing")

return t.suite("runtime", {
  tests = {
    t.test("raises", function()
      error("runtime exploded")
    end),
  },
})
