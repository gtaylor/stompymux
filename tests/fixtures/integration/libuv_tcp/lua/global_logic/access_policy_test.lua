local AccessPolicy = require("access_policy")

local POLICY_OPTIONS = {
  namespace = "locks.traverse",
  enactor_message = "default enactor denial",
  other_message = "default others denial",
}

local function traverse_passes(exit, enactor, subject)
  return mux.world.lock_passes({
    object = exit,
    enactor = enactor,
    cause = enactor,
    subject = subject,
    lock = mux.world.locks.TRAVERSE,
  })
end

local function evaluate(exit, subject)
  return AccessPolicy.evaluate({ object = exit:dbref(), subject = subject:dbref() }, POLICY_OPTIONS)
end

local function access_policy_test(ctx)
  local enactor = mux.world.object(ctx.enactor)
  local room = mux.world.create_object({
    type = mux.world.types.ROOM,
    name = "Access Policy Test Room",
  })
  local subject = mux.world.create_object({
    type = mux.world.types.THING,
    name = "Access Policy Test Subject",
    location = room,
  })
  local exit = mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "Access Policy Test Exit",
    location = room,
    destination = room,
  })
  local affiliation = mux.world.create_object({
    type = mux.world.types.THING,
    name = "Access Policy Test Affiliation",
    location = room,
  })
  local other_affiliation = mux.world.create_object({
    type = mux.world.types.THING,
    name = "Other Access Policy Affiliation",
    location = room,
  })
  local policy = exit:state("locks.traverse")

  assert(traverse_passes(exit, enactor, subject))

  policy:set("flag/SAFE", true)
  assert(not traverse_passes(exit, enactor, subject))
  subject:flags():add(mux.world.flags.SAFE)
  assert(traverse_passes(exit, enactor, subject))
  policy:set("flag/SAFE", false)
  assert(not traverse_passes(exit, enactor, subject))
  subject:flags():remove(mux.world.flags.SAFE)
  assert(traverse_passes(exit, enactor, subject))

  policy:set("flag/SAFE", nil)
  policy:set("flag/WIZARD", true)
  assert(not traverse_passes(exit, enactor, subject))
  assert(traverse_passes(exit, enactor, enactor))

  policy:set("flag/WIZARD", nil)
  subject:set_affiliation(affiliation)
  policy:set("affiliation", affiliation:dbref())
  assert(traverse_passes(exit, enactor, subject))
  policy:set("affiliation", other_affiliation:dbref())
  assert(not traverse_passes(exit, enactor, subject))

  policy:set("affiliation", nil)
  policy:set("attribute/Mechname", "Test subject")
  assert(not traverse_passes(exit, enactor, subject))
  subject:attributes():set("Mechname", "Test subject")
  assert(traverse_passes(exit, enactor, subject))

  policy:set("attribute/Mechname", nil)
  policy:set("state/access/enabled", true)
  assert(not traverse_passes(exit, enactor, subject))
  subject:state("access"):set("enabled", true)
  assert(traverse_passes(exit, enactor, subject))
  policy:set("state/access/level", 7)
  subject:state("access"):set("level", "7")
  assert(not traverse_passes(exit, enactor, subject))
  subject:state("access"):set("level", 7)
  assert(traverse_passes(exit, enactor, subject))

  policy:set("flag/SAFE", true)
  assert(not traverse_passes(exit, enactor, subject))
  subject:flags():add(mux.world.flags.SAFE)
  assert(traverse_passes(exit, enactor, subject))

  subject:state("access"):set("level", 8)
  local denied = evaluate(exit, subject)
  assert(not denied.passes)
  assert(denied.enactor_message == "default enactor denial")
  assert(denied.other_message == "default others denial")
  denied = AccessPolicy.evaluate(
    { object = exit:dbref(), subject = subject:dbref() },
    { namespace = "locks.traverse", enactor_message = "enactor only" }
  )
  assert(denied.enactor_message == "enactor only" and denied.other_message == nil)
  policy:set("message/enactor", "custom enactor denial")
  policy:set("message/others", "custom others denial")
  denied = evaluate(exit, subject)
  assert(denied.enactor_message == "custom enactor denial")
  assert(denied.other_message == "custom others denial")

  policy:set("message/enactor", nil)
  policy:set("message/others", nil)
  policy:set("malformed", true)
  assert(not traverse_passes(exit, enactor, subject))
  local ok, err = pcall(evaluate, exit, subject)
  assert(not ok and tostring(err):find("invalid access policy", 1, true))

  policy:set("malformed", nil)
  policy:set("flag/NOT_A_FLAG", true)
  ok, err = pcall(evaluate, exit, subject)
  assert(not ok and tostring(err):find("unsupported flag", 1, true))

  policy:set("flag/NOT_A_FLAG", nil)
  policy:set("attribute/Mechname", true)
  ok, err = pcall(evaluate, exit, subject)
  assert(not ok and tostring(err):find("attribute requirement must be a string", 1, true))

  subject:flags():remove(mux.world.flags.SAFE)
  mux.world.destroy_object(exit)
  mux.world.destroy_object(subject)
  mux.world.destroy_object(affiliation)
  mux.world.destroy_object(other_affiliation)
  mux.world.destroy_object(room)
  mux.world.pemit(ctx.enactor, "AccessPolicy passed")
  return true
end

return {
  commands = {
    {
      pattern = "^accesspolicytest$",
      access = mux.world.access.WIZARD,
      handler = access_policy_test,
    },
  },
}
