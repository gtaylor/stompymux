-- Opt-in example tactical encounter.
--
-- Requiring this module has no game effect.  A caller explicitly creates and
-- starts an encounter, owns the listed units, and calls tick once per
-- simulation tick.  The example evaluates its director at most once per
-- three seconds of simulation time.

local btech = require("btech")
local director = require("tactical_director")
local example = {}

local function fail(message)
  error(message, 0)
end

local function copy_table(source)
  local result = {}
  for key, value in pairs(source or {}) do
    result[key] = value
  end
  return result
end

local function validate_units(units)
  if type(units) ~= "table" or #units == 0 then
    fail("encounter units must be a non-empty array")
  end
  local seen = {}
  local result = {}
  for _, unit in ipairs(units) do
    if
      type(unit) ~= "number"
      or unit ~= unit
      or unit == math.huge
      or unit == -math.huge
      or unit ~= math.floor(unit)
      or unit < 0
    then
      fail("encounter unit IDs must be non-negative integers")
    end
    if seen[unit] then
      fail("encounter units must not contain duplicates")
    end
    seen[unit] = true
    result[#result + 1] = unit
  end
  return result
end

local function message(error_value)
  if type(error_value) == "table" then
    local value = error_value.message or error_value.code or error_value.detail or "Lua error"
    return type(value) == "string" and value or tostring(value)
  end
  return tostring(error_value)
end

local function is_revision_conflict(error_value)
  local text = string.lower(message(error_value))
  return string.find(text, "revision", 1, true) ~= nil or string.find(text, "stale", 1, true) ~= nil
end

local function copy_nested_table(source)
  local result = {}
  for key, value in pairs(source or {}) do
    if type(value) == "table" then
      result[key] = copy_nested_table(value)
    else
      result[key] = value
    end
  end
  return result
end

local function merge_roster_snapshots(encounter, snapshots, unavailable)
  local snapshot = {
    version = 1,
    time = encounter.last_observation_time or 0,
    units = {},
    contacts = {},
    roster_incomplete = #unavailable > 0,
  }
  local contacts = {}
  for _, source in ipairs(snapshots) do
    if snapshot.time == 0 or source.time > snapshot.time then
      snapshot.time = source.time
    end
    for _, entry in ipairs(source.units or {}) do
      snapshot.units[#snapshot.units + 1] = entry
    end
    for _, contact in ipairs(source.contacts or {}) do
      local merged = contacts[contact.unit]
      if not merged then
        merged = { unit = contact.unit, observations = {} }
        contacts[contact.unit] = merged
        snapshot.contacts[#snapshot.contacts + 1] = merged
      end
      for _, sighting in ipairs(contact.observations or {}) do
        merged.observations[#merged.observations + 1] = copy_nested_table(sighting)
      end
    end
  end
  for _, entry in ipairs(unavailable) do
    snapshot.units[#snapshot.units + 1] = entry
  end
  table.sort(snapshot.units, function(left, right)
    return left.unit < right.unit
  end)
  table.sort(snapshot.contacts, function(left, right)
    return left.unit < right.unit
  end)
  for _, contact in ipairs(snapshot.contacts) do
    table.sort(contact.observations, function(left, right)
      return left.observer < right.observer
    end)
  end
  return snapshot
end

-- The native group API deliberately rejects a detached, unplaced, or
-- cross-map member.  Preserve that strict contract, but let this opt-in
-- example report the assigned member as unavailable and stop issuing a
-- partial batch while the caller repairs the roster.
local function observe_roster(encounter)
  local ok, snapshot = pcall(function()
    return btech.tactical.observe(encounter.units, encounter.feedback_cursors)
  end)
  if ok then
    snapshot.roster_incomplete = false
    return snapshot
  end

  local snapshots = {}
  local unavailable = {}
  for _, unit in ipairs(encounter.units) do
    local cursor = {}
    if encounter.feedback_cursors[unit] ~= nil then
      cursor[unit] = encounter.feedback_cursors[unit]
    end
    local member_ok, member_snapshot = pcall(function()
      return btech.tactical.observe({ unit }, cursor)
    end)
    if member_ok and member_snapshot.units and member_snapshot.units[1] then
      local entry = member_snapshot.units[1]
      local position = entry.observation and entry.observation.position
      if position and position.map ~= encounter.objective.map then
        unavailable[#unavailable + 1] = { unit = unit, error = "map_changed" }
      else
        snapshots[#snapshots + 1] = member_snapshot
      end
    else
      unavailable[#unavailable + 1] = { unit = unit, error = message(member_snapshot) }
    end
  end
  return merge_roster_snapshots(encounter, snapshots, unavailable)
end

local function advance_feedback_cursors(encounter, snapshot)
  for _, entry in ipairs(snapshot.units or {}) do
    local records = entry.feedback and entry.feedback.records or {}
    for _, record in ipairs(records) do
      if record.sequence and record.sequence > (encounter.feedback_cursors[entry.unit] or 0) then
        encounter.feedback_cursors[entry.unit] = record.sequence
      end
    end
  end
end

---Create an inert encounter controller.
---
---`options.config` is passed to attach, and `options.configure` is applied as
---a second explicit management step in `start`.  Neither operation occurs
---until the caller invokes `start`.
---@param units integer[] Caller-owned unit IDs.
---@param objective table Shared map/x/y objective.
---@param options table|nil Configuration passed by the caller.
---@return table encounter Inert encounter state.
function example.create(units, objective, options)
  return {
    units = validate_units(units),
    objective = copy_table(objective),
    options = copy_table(options),
    interval = 3,
    ticks = 0,
    started = false,
    state = nil,
    feedback_cursors = {},
    last_observation_time = nil,
    last_evaluation_time = nil,
    last_roster_incomplete = nil,
  }
end

---Attach, configure, and resume each caller-selected unit.
---@param encounter table Encounter returned by create.
---@return table encounter The started encounter.
function example.start(encounter)
  if type(encounter) ~= "table" or encounter.started then
    fail("encounter is already started or invalid")
  end
  for _, unit in ipairs(encounter.units) do
    btech.autopilot.attach(unit, encounter.options.config or {})
    btech.autopilot.configure(unit, encounter.options.configure or {})
    btech.autopilot.resume(unit)
  end
  encounter.started = true
  return encounter
end

local function evaluate(encounter, snapshot)
  local intentions, state = director.plan(snapshot, encounter.objective, encounter.state)
  local submitted = {}
  if #intentions > 0 then
    local ok, result = pcall(function()
      return btech.tactical.submit(intentions)
    end)
    if not ok and is_revision_conflict(result) then
      -- A battlefield director may have raced a manual or another tactical
      -- update.  Re-read revisions and plan once; no blind retry is allowed.
      snapshot = observe_roster(encounter)
      intentions, state = director.plan(snapshot, encounter.objective, encounter.state)
      if #intentions > 0 then
        result = btech.tactical.submit(intentions)
      else
        result = {}
      end
    elseif not ok then
      error(result, 0)
    end
    submitted = result
  end
  -- Keep the cursors pinned while a member is unavailable.  Otherwise a
  -- successful order could be consumed from the remaining singleton pages
  -- before the full roster is restored and the director can account for it.
  if not snapshot.roster_incomplete then
    advance_feedback_cursors(encounter, snapshot)
  end
  encounter.state = state
  encounter.last_evaluation_time = snapshot.time
  return {
    evaluated = true,
    time = snapshot.time,
    snapshot = snapshot,
    intentions = intentions,
    submitted = submitted,
    state = state,
  }
end

---Advance one simulation tick; evaluate only every three simulation seconds.
---@param encounter table Started encounter.
---@return table result Tick result, with `evaluated=false` between director passes.
function example.tick(encounter)
  if type(encounter) ~= "table" or not encounter.started then
    fail("encounter must be started before ticking")
  end
  encounter.ticks = encounter.ticks + 1
  local snapshot = observe_roster(encounter)
  local time = snapshot.time
  local roster_changed = encounter.last_roster_incomplete ~= snapshot.roster_incomplete
  if encounter.last_observation_time == time and not roster_changed then
    return { evaluated = false, tick = encounter.ticks, time = time, snapshot = snapshot }
  end
  encounter.last_observation_time = time
  encounter.last_roster_incomplete = snapshot.roster_incomplete
  if
    encounter.last_evaluation_time ~= nil
    and time >= encounter.last_evaluation_time
    and time - encounter.last_evaluation_time < encounter.interval
    and not roster_changed
  then
    return { evaluated = false, tick = encounter.ticks, time = time, snapshot = snapshot }
  end
  local result = evaluate(encounter, snapshot)
  result.tick = encounter.ticks
  return result
end

return example
