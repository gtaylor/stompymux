local AccessPolicy = require("access_policy")

local function traverse_lock(ctx)
  return AccessPolicy.evaluate(ctx, {
    namespace = "locks.traverse",
    enactor_message = "You cannot go that way.",
  })
end

return {
  locks = {
    traverse = traverse_lock,
  },
}
