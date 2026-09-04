local t = require("testing")

return t.suite("passing", {
  tests = {
    t.test("works", function(_, expect)
      local codes = mux.error.codes
      expect.equal(2, 2)
      expect.equal(btech.error.codes.unavailable.code, "btech.unavailable")
      expect.raises_code(function()
        mux.error.raise(btech.error.codes.failed, "expected")
      end, btech.error.codes)
      expect.raises_code(function()
        mux.error.raise("author.example", "expected")
      end, "author")
      expect.raises_code(function()
        mux.error.raise(codes.state.invalid, "expected")
      end, codes.state)
      local ok, err = mux.error.pcall(function()
        error("plain runtime error")
      end)
      expect.falsy(ok)
      expect.is_error(err, codes.runtime)
      ---@cast err Error
      expect.truthy(type(err.traceback) == "string" and #err.traceback > 0)
      local cause = mux.error.new({ code = "author.cause", message = "cause" })
      local wrapped = mux.error.wrap(cause, "author.wrapper", "wrapper")
      expect.equal(wrapped:root(), cause)
      expect.truthy(wrapped:is("author"))
      local value, expected_error = nil, mux.error.new({
        code = "author.expected",
        message = "expected failure",
      })
      expect.is_nil(value)
      expect.is_error(expected_error, "author.expected")
    end),
  },
})
