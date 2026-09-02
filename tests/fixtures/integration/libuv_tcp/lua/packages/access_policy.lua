-- Declarative access policies stored in an object's persistent state. Every
-- requirement in the selected namespace is combined with AND. Supported keys:
--
--   flag/<FLAG>                 boolean flag-presence comparison
--   affiliation                integer dbref identity comparison
--   attribute/<Name>           exact native-attribute string comparison
--   state/<namespace>/<key>    exact typed persistent-state comparison
--   message/enactor            optional denial message for the enactor
--   message/others             optional denial message for nearby objects

local access_policy = {}

---Raises a consistently formatted configuration error for one policy entry.
---@param namespace string
---@param key string
---@param message string
local function policy_error(namespace, key, message)
  error(string.format("invalid access policy %q key %q: %s", namespace, key, message), 0)
end

---Resolves a canonical flag name or raises a policy-specific error.
---@param namespace string
---@param key string
---@param name string
---@return Flag
local function checked_flag(namespace, key, name)
  local ok, flag = pcall(function()
    return mux.world.flags[name]
  end)

  if not ok then
    policy_error(namespace, key, "unsupported flag " .. string.format("%q", name))
  end
  return flag
end

---Reads a supported native attribute or raises a policy-specific error.
---@param namespace string
---@param key string
---@param attributes Attribute
---@param name string
---@return string?
local function checked_attribute(namespace, key, attributes, name)
  local ok, value = pcall(attributes.get, attributes, name)

  if not ok then
    policy_error(namespace, key, "unsupported attribute " .. string.format("%q", name))
  end
  return value
end

---Resolves a live affiliation dbref or raises a policy-specific error.
---@param namespace string
---@param key string
---@param dbref StateValue
---@return Object
local function checked_affiliation(namespace, key, dbref)
  if type(dbref) ~= "number" or dbref < 0 or dbref ~= math.floor(dbref) then
    policy_error(namespace, key, "affiliation must be a non-negative integer dbref")
  end

  local ok, object = pcall(mux.world.object, dbref)
  if not ok then
    policy_error(namespace, key, "affiliation dbref does not identify a live object")
  end
  return object
end

---Compares both the Lua scalar type and value, keeping `7` distinct from `"7"`.
---@param actual StateValue|nil
---@param expected StateValue
---@return boolean
local function scalar_equals(actual, expected)
  return type(actual) == type(expected) and actual == expected
end

---Builds the structured failure result expected by a Lua lock callback.
---@param enactor_message? string
---@param other_message? string
---@return { passes: false, enactor_message?: string, other_message?: string }
local function deny(enactor_message, other_message)
  return {
    passes = false,
    enactor_message = enactor_message,
    other_message = other_message,
  }
end

---Evaluates an object's state-backed policy against the lock subject.
---
---All policy entries are inspected even after a requirement mismatches, so a
---malformed later entry still raises an error instead of remaining hidden.
---Message-only policies pass because message entries are metadata rather than
---requirements. Invalid policy data raises an error and therefore fails closed
---when this function is used from a native Lua lock callback.
---@param ctx { object: DbRef, subject: DbRef } Lua lock context containing object and subject dbrefs.
---@param options { namespace: string, enactor_message?: string, other_message?: string } Policy namespace and optional default messages.
---@return true|{ passes: false, enactor_message?: string, other_message?: string } result `true` on success, otherwise a structured denial.
function access_policy.evaluate(ctx, options)
  assert(type(ctx) == "table", "access policy context must be a table")
  assert(type(options) == "table", "access policy options must be a table")
  assert(type(options.namespace) == "string", "access policy namespace must be a string")
  assert(
    options.enactor_message == nil or type(options.enactor_message) == "string",
    "default enactor message must be a string or nil"
  )
  assert(
    options.other_message == nil or type(options.other_message) == "string",
    "default other message must be a string or nil"
  )

  local namespace = options.namespace
  local policy_object = mux.world.object(ctx.object)
  local subject = mux.world.object(ctx.subject)
  local subject_attributes = subject:attributes()
  local entries = policy_object:state(namespace):entries()
  local passes = true
  local enactor_message = options.enactor_message
  local other_message = options.other_message

  for _, entry in ipairs(entries) do
    local key = entry.key
    local value = entry.value
    local flag_name = string.match(key, "^flag/([A-Za-z][A-Za-z0-9_.%-]*)$")
    local attribute_name = string.match(key, "^attribute/([A-Za-z][A-Za-z0-9_.%-]*)$")
    local state_namespace, state_key =
      string.match(key, "^state/([A-Za-z][A-Za-z0-9_.%-]*)/([A-Za-z][A-Za-z0-9_.%-]*)$")

    if flag_name then
      if type(value) ~= "boolean" then
        policy_error(namespace, key, "flag requirement must be a boolean")
      end
      local flag = checked_flag(namespace, key, flag_name)
      if subject:flags():has(flag) ~= value then
        passes = false
      end
    elseif key == "affiliation" then
      local required_affiliation = checked_affiliation(namespace, key, value)
      if subject:affiliation() ~= required_affiliation then
        passes = false
      end
    elseif attribute_name then
      if type(value) ~= "string" then
        policy_error(namespace, key, "attribute requirement must be a string")
      end
      if checked_attribute(namespace, key, subject_attributes, attribute_name) ~= value then
        passes = false
      end
    elseif state_namespace then
      local actual = subject:state(state_namespace):get(state_key)
      if not scalar_equals(actual, value) then
        passes = false
      end
    elseif key == "message/enactor" then
      if type(value) == "string" then
        enactor_message = value
      else
        policy_error(namespace, key, "enactor message must be a string")
      end
    elseif key == "message/others" then
      if type(value) == "string" then
        other_message = value
      else
        policy_error(namespace, key, "others message must be a string")
      end
    else
      policy_error(namespace, key, "unknown or malformed policy entry")
    end
  end

  if passes then
    return true
  end
  return deny(enactor_message, other_message)
end

return access_policy
