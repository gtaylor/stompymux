local testing = {}

local function display(value)
  if value == nil then
    return "nil"
  end
  return tostring(value)
end

local function failure(message, expected, actual)
  error({
    kind = "assertion",
    message = message,
    expected = expected,
    actual = actual,
  }, 0)
end

local expect = {}

function expect.equal(actual, expected)
  if actual ~= expected then
    failure("expected " .. display(expected) .. ", got " .. display(actual), expected, actual)
  end
end

function expect.not_equal(actual, expected)
  if actual == expected then
    failure("did not expect " .. display(actual), "not " .. display(expected), actual)
  end
end

function expect.truthy(actual)
  if not actual then
    failure("expected a truthy value, got " .. display(actual), true, actual)
  end
end

function expect.falsy(actual)
  if actual then
    failure("expected a falsy value, got " .. display(actual), false, actual)
  end
end

function expect.is_nil(actual)
  if actual ~= nil then
    failure("expected nil, got " .. display(actual), nil, actual)
  end
end

function expect.contains(actual, expected)
  local contains = false

  if type(actual) == "string" then
    contains = string.find(actual, expected, 1, true) ~= nil
  elseif type(actual) == "table" then
    for _, value in pairs(actual) do
      if value == expected then
        contains = true
        break
      end
    end
  end
  if not contains then
    failure("expected " .. display(actual) .. " to contain " .. display(expected), expected, actual)
  end
end

function expect.near(actual, expected, tolerance)
  tolerance = tolerance or 1e-9
  if
    type(actual) ~= "number"
    or type(expected) ~= "number"
    or type(tolerance) ~= "number"
    or math.abs(actual - expected) > tolerance
  then
    failure(
      "expected " .. display(actual) .. " to be within " .. display(tolerance) .. " of " .. display(expected),
      expected,
      actual
    )
  end
end

function expect.error_matches(callback, pattern)
  local ok, message = pcall(callback)

  if ok then
    failure("expected function to raise an error matching " .. display(pattern), pattern, nil)
  end
  if not string.find(tostring(message), pattern) then
    failure("error did not match " .. display(pattern) .. ": " .. tostring(message), pattern, message)
  end
end

function testing.test(name, callback)
  assert(type(name) == "string" and name ~= "", "test name must be a non-empty string")
  assert(type(callback) == "function", "test callback must be a function")
  return { name = name, run = callback }
end

function testing.suite(name, definition)
  assert(type(name) == "string" and name ~= "", "suite name must be a non-empty string")
  assert(type(definition) == "table", "suite definition must be a table")
  assert(type(definition.tests) == "table", "suite tests must be a table")
  definition.name = name
  definition.expect = expect
  return definition
end

return testing
