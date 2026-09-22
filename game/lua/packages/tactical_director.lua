-- Pure, deterministic tactical planning helpers.
--
-- This module deliberately does not read the world or submit orders.  A caller
-- obtains a snapshot with btech.tactical.observe, passes it to plan, and then
-- submits the returned intentions with btech.tactical.submit.  Keeping planning
-- separate from admission lets a future director replace this policy without
-- giving it tick-level control of the simulation.

local btech = require("btech")
local tactical_director = {}

local function fail(message)
  error(message, 0)
end

local function integer(value, name, minimum)
  if
    type(value) ~= "number"
    or value ~= value
    or value == math.huge
    or value == -math.huge
    or value ~= math.floor(value)
    or value < (minimum or 0)
  then
    fail(name .. " must be a non-negative integer")
  end
  return value
end

local function copy_position(position)
  return { map = position.map, x = position.x, y = position.y }
end

local function copy_objective(objective)
  return {
    map = objective.map,
    x = objective.x,
    y = objective.y,
    arrival_radius = objective.arrival_radius or 0,
  }
end

local function same_objective(left, right)
  return left
    and right
    and left.map == right.map
    and left.x == right.x
    and left.y == right.y
    and (left.arrival_radius or 0) == (right.arrival_radius or 0)
end

local function check_objective(objective)
  if type(objective) ~= "table" then
    fail("objective must be a table")
  end
  integer(objective.map, "objective.map")
  integer(objective.x, "objective.x")
  integer(objective.y, "objective.y")
  integer(objective.arrival_radius or 0, "objective.arrival_radius")
  return copy_objective(objective)
end

-- Battle maps use zero-based column-staggered coordinates.  This matches the
-- native Hex::distance implementation and keeps arrival reporting consistent
-- with the movement executor.
local function hex_distance(left, right)
  local function cube(position)
    local q = position.x
    local r = position.y - (q + q % 2) / 2
    return q, r, -q - r
  end
  local aq, ar, as = cube(left)
  local bq, br, bs = cube(right)
  return math.max(math.abs(aq - bq), math.abs(ar - br), math.abs(as - bs))
end

local function same_position(left, right)
  return left and right and left.map == right.map and left.x == right.x and left.y == right.y
end

local function matches_objective(order, objective)
  return order
    and (order.kind == "attack_move" or order.kind == btech.autopilot.orders.ATTACK_MOVE)
    and same_position(order.destination, objective)
    and (order.arrival_radius or 0) == objective.arrival_radius
end

local function has_matching_order(status, objective)
  local active = status.active
  if active and matches_objective(active.order, objective) then
    return active.state == nil or active.state == "running"
  end
  -- A queued order is the current intention only when there is no active
  -- order.  In particular, do not preserve an attack-move hidden behind an
  -- unrelated Hold forever.
  if not active then
    local first = (status.queue or {})[1]
    return first and matches_objective(first.order, objective) and (first.state == nil or first.state == "queued")
  end
  return false
end

local function copy_assignments(assignments)
  local result = {}
  for unit, assignment in pairs(assignments or {}) do
    result[unit] = {
      id = assignment.id,
      outcome = assignment.outcome,
    }
  end
  return result
end

local function copy_report(report)
  local result = {
    partial = report.partial,
    complete = report.complete,
    success = report.success,
    planned = {},
    preserved = {},
    arrivals = {},
    unavailable = {},
    skipped = {},
    history_gaps = {},
  }
  for _, key in ipairs({ "planned", "preserved", "arrivals", "unavailable", "skipped", "history_gaps" }) do
    for index, value in ipairs(report[key]) do
      result[key][index] = value
    end
  end
  return result
end

local function unit_result(report, key, unit, reason)
  report[key][#report[key] + 1] = { unit = unit, reason = reason }
end

local function mark_feedback(assignment, feedback)
  if not assignment or assignment.id ~= feedback.order_id then
    return
  end
  if feedback.event == "order_succeeded" then
    assignment.outcome = "succeeded"
  elseif feedback.event == "order_failed" then
    assignment.outcome = "failed"
  elseif feedback.event == "order_canceled" then
    assignment.outcome = "canceled"
  end
end

local function active_assignment(status, objective)
  local active = status.active
  if active and matches_objective(active.order, objective) and active.state ~= "failed" then
    return { id = active.id, outcome = "pending" }
  end
  if not active then
    local first = (status.queue or {})[1]
    if first and matches_objective(first.order, objective) and (first.state == nil or first.state == "queued") then
      return { id = first.id, outcome = "pending" }
    end
  end
  return nil
end

local function assignment_succeeded(assignment)
  return assignment and assignment.outcome == "succeeded"
end

local function has_pending_order(status)
  return status.active ~= nil or #(status.queue or {}) > 0
end

local function is_stopped(observation)
  return math.abs(observation.speed or 0) <= 0.1
end

---Build attack-move intentions for a shared objective.
---
---The input snapshot must come from `btech.tactical.observe`.  The function is
---read-only and deterministic: it only examines the supplied status and
---observation tables.  Paused or blocked controllers are left untouched;
---matching attack-move orders are preserved; units already in the objective
---region are reported as arrivals.
---@param snapshot table Versioned tactical observation snapshot.
---@param objective table Shared map/x/y objective and optional arrival radius.
---@param state table|nil Previous director state.
---@return table[] intentions Per-unit replacement intentions.
---@return table new_state Updated state and outcome report.
function tactical_director.plan(snapshot, objective, state)
  if type(snapshot) ~= "table" or snapshot.version ~= 1 then
    fail("snapshot.version must be 1")
  end
  if type(snapshot.units) ~= "table" then
    fail("snapshot.units must be a table")
  end
  objective = check_objective(objective)
  if state ~= nil and type(state) ~= "table" then
    fail("state must be a table or nil")
  end

  local intentions = {}
  local report = {
    partial = false,
    complete = false,
    success = false,
    planned = {},
    preserved = {},
    arrivals = {},
    unavailable = {},
    skipped = {},
    history_gaps = {},
  }
  local assignments = copy_assignments(state and same_objective(state.objective, objective) and state.assignments)
  local required_units = {}
  if state and state.units then
    for _, unit in ipairs(state.units) do
      required_units[unit] = true
    end
  else
    for _, entry in ipairs(snapshot.units) do
      required_units[entry.unit] = true
    end
  end
  local new_state = {
    cycle = (state and state.cycle or 0) + 1,
    objective = copy_objective(objective),
    units = {},
    assignments = assignments,
    report = report,
  }
  for _, unit in ipairs(state and state.units or {}) do
    new_state.units[#new_state.units + 1] = unit
  end

  for _, entry in ipairs(snapshot.units) do
    local unit = entry.unit
    if type(unit) ~= "number" then
      fail("snapshot unit IDs must be numbers")
    end
    local status = entry.status
    local observation = entry.observation
    local known = false
    for _, known_unit in ipairs(new_state.units) do
      known = known or known_unit == unit
    end
    if not known then
      new_state.units[#new_state.units + 1] = unit
    end
    required_units[unit] = true
    if type(status) ~= "table" or type(observation) ~= "table" then
      report.partial = true
      unit_result(report, "unavailable", unit, entry.error or "unavailable")
    elseif status.state == "paused" or status.state == "blocked" then
      report.partial = true
      unit_result(report, "skipped", unit, status.state)
    elseif not observation.position then
      report.partial = true
      unit_result(report, "unavailable", unit, "unplaced")
    elseif observation.position.map ~= objective.map then
      report.partial = true
      unit_result(report, "unavailable", unit, "map_changed")
    elseif snapshot.roster_incomplete then
      -- A tactical group is an atomic planning input.  When the encounter
      -- wrapper had to omit an unavailable member, wait for the caller to
      -- repair that roster instead of issuing a partial batch.
      report.partial = true
      unit_result(report, "skipped", unit, "roster_incomplete")
    else
      local assignment = assignments[unit]
      local page = entry.feedback or {}
      if page.history_gap then
        assignments[unit] = nil
        assignment = nil
        report.partial = true
        unit_result(report, "history_gaps", unit, "feedback_gap")
      else
        for _, feedback in ipairs(page.records or {}) do
          mark_feedback(assignment, feedback)
        end
      end

      local current = active_assignment(status, objective)
      if current then
        assignments[unit] = current
        assignment = current
      end
      local at_goal = hex_distance(observation.position, objective) <= objective.arrival_radius
      local stopped = is_stopped(observation)
      if at_goal and stopped and not has_pending_order(status) then
        if assignment and not assignment_succeeded(assignment) then
          report.partial = true
          unit_result(report, "unavailable", unit, assignment.outcome or "order_not_succeeded")
        else
          unit_result(report, "arrivals", unit, "arrived")
        end
      elseif has_matching_order(status, objective) then
        unit_result(report, "preserved", unit, "matching_order")
      elseif assignment and assignment.outcome == "succeeded" and at_goal and stopped then
        unit_result(report, "arrivals", unit, "succeeded")
      else
        intentions[#intentions + 1] = {
          unit = unit,
          expected_revision = status.revision,
          mode = btech.autopilot.submission_modes.REPLACE,
          orders = {
            {
              kind = btech.autopilot.orders.ATTACK_MOVE,
              destination = copy_position(objective),
              arrival_radius = objective.arrival_radius,
            },
          },
        }
        unit_result(report, "planned", unit, "attack_move")
      end
    end
  end

  for unit in pairs(required_units) do
    local present = false
    for _, entry in ipairs(snapshot.units) do
      present = present or entry.unit == unit
    end
    if not present then
      report.partial = true
      unit_result(report, "unavailable", unit, "missing_snapshot_unit")
    end
  end
  local complete = true
  for unit in pairs(required_units) do
    local arrived = false
    for _, item in ipairs(report.arrivals) do
      arrived = arrived or item.unit == unit
    end
    if not arrived then
      complete = false
      break
    end
  end
  report.complete = complete and next(required_units) ~= nil
  report.success = report.complete and not report.partial
  new_state.report = copy_report(report)
  return intentions, new_state
end

return tactical_director
