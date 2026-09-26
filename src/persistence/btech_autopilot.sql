-- Rust autopilot controllers, one typed row per controller plus child rows for its
-- orders, patrol waypoints and feedback history. Sensor memory is never stored.
--
-- Codes: fire_mode 0 hold, 1 assigned target, 2 opportunistic. state 0 paused, 1 idle,
-- 2 executing, 3 blocked. Reasons 0-11 and feedback events 0-9 follow the declaration
-- order of AutopilotReason and AutopilotFeedbackEvent. Order kind 0 move, 1 hold,
-- 2 follow, 3 patrol, 4 attack, 5 attack-move; order state 0 queued, 1 running,
-- 2 succeeded, 3 failed, 4 canceled.
CREATE TABLE IF NOT EXISTS btech_autopilot_controllers (
    unit_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    speed_percent INTEGER NOT NULL CHECK (speed_percent BETWEEN 0 AND 100),
    fire_mode INTEGER NOT NULL CHECK (fire_mode BETWEEN 0 AND 2),
    heat_ceiling INTEGER NOT NULL CHECK (heat_ceiling BETWEEN 0 AND 1000),
    range_minimum INTEGER CHECK (range_minimum BETWEEN 0 AND 65535),
    range_maximum INTEGER CHECK (range_maximum BETWEEN 0 AND 65535),
    state INTEGER NOT NULL CHECK (state BETWEEN 0 AND 3),
    blocking_reason INTEGER CHECK (blocking_reason BETWEEN 0 AND 11),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    next_order_id INTEGER NOT NULL CHECK (next_order_id > 0),
    next_feedback_sequence INTEGER NOT NULL CHECK (next_feedback_sequence > 0),
    CHECK ((range_minimum IS NULL) = (range_maximum IS NULL)),
    CHECK (range_minimum <= range_maximum),
    CHECK (state <> 3 OR blocking_reason IS NOT NULL)
) STRICT;

-- The active order has a NULL queue_position; queued orders count up from zero.
CREATE TABLE IF NOT EXISTS btech_autopilot_controller_orders (
    unit_dbref INTEGER NOT NULL REFERENCES btech_autopilot_controllers(unit_dbref) ON DELETE CASCADE,
    order_id INTEGER NOT NULL CHECK (order_id > 0),
    queue_position INTEGER CHECK (queue_position BETWEEN 0 AND 63),
    kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 5),
    state INTEGER NOT NULL CHECK (state BETWEEN 0 AND 4),
    destination_map INTEGER CHECK (destination_map >= 0),
    destination_x INTEGER CHECK (destination_x BETWEEN 0 AND 65535),
    destination_y INTEGER CHECK (destination_y BETWEEN 0 AND 65535),
    arrival_radius INTEGER CHECK (arrival_radius BETWEEN 0 AND 65535),
    target_dbref INTEGER CHECK (target_dbref >= 0),
    separation INTEGER CHECK (separation BETWEEN 1 AND 65535),
    range_minimum INTEGER CHECK (range_minimum BETWEEN 0 AND 65535),
    range_maximum INTEGER CHECK (range_maximum BETWEEN 0 AND 65535),
    waypoint_index INTEGER NOT NULL CHECK (waypoint_index BETWEEN 0 AND 65535),
    recovery_attempts INTEGER NOT NULL CHECK (recovery_attempts BETWEEN 0 AND 255),
    stagnant_ticks INTEGER NOT NULL CHECK (stagnant_ticks BETWEEN 0 AND 65535),
    origin_map INTEGER,
    origin_x INTEGER CHECK (origin_x BETWEEN 0 AND 65535),
    origin_y INTEGER CHECK (origin_y BETWEEN 0 AND 65535),
    suppressed_target INTEGER,
    PRIMARY KEY (unit_dbref, order_id),
    CHECK ((kind IN (0, 5)) = (destination_map IS NOT NULL)),
    CHECK ((destination_map IS NULL) = (destination_x IS NULL)),
    CHECK ((destination_map IS NULL) = (destination_y IS NULL)),
    CHECK ((destination_map IS NULL) = (arrival_radius IS NULL)),
    CHECK ((kind IN (2, 4)) = (target_dbref IS NOT NULL)),
    CHECK ((kind = 2) = (separation IS NOT NULL)),
    CHECK (kind = 4 OR range_minimum IS NULL),
    CHECK ((range_minimum IS NULL) = (range_maximum IS NULL)),
    CHECK (range_minimum <= range_maximum),
    CHECK ((origin_map IS NULL) = (origin_x IS NULL)),
    CHECK ((origin_map IS NULL) = (origin_y IS NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS btech_autopilot_controller_waypoints (
    unit_dbref INTEGER NOT NULL,
    order_id INTEGER NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 63),
    map_dbref INTEGER NOT NULL CHECK (map_dbref >= 0),
    x INTEGER NOT NULL CHECK (x BETWEEN 0 AND 65535),
    y INTEGER NOT NULL CHECK (y BETWEEN 0 AND 65535),
    PRIMARY KEY (unit_dbref, order_id, position),
    FOREIGN KEY (unit_dbref, order_id)
        REFERENCES btech_autopilot_controller_orders(unit_dbref, order_id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS btech_autopilot_controller_feedback (
    unit_dbref INTEGER NOT NULL REFERENCES btech_autopilot_controllers(unit_dbref) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    simulation_time INTEGER NOT NULL,
    order_id INTEGER CHECK (order_id >= 0),
    event INTEGER NOT NULL CHECK (event BETWEEN 0 AND 9),
    reason INTEGER CHECK (reason BETWEEN 0 AND 11),
    PRIMARY KEY (unit_dbref, sequence)
) STRICT, WITHOUT ROWID;
