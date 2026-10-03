-- Rust-owned BattleTech tables, installed with the schema-32 tables when a database is
-- created and required by every later open. Each table is documented where it is
-- defined; the modules that read and write it are named in the section headers.
--
-- Countdowns are stored as deadlines on the simulation clock: the simulation second at
-- which they end (see btech_deadlines.rs). A timer counting down in step with the clock
-- keeps the same deadline, so its row is written only when it starts, is rescheduled
-- or finishes.

-- btech_turn_clock.rs: elapsed simulation time and the turn phase's fixed offset from it.
CREATE TABLE btech_simulation_clock (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    seconds INTEGER NOT NULL CHECK (seconds >= 0),
    phase_offset INTEGER NOT NULL CHECK (phase_offset BETWEEN 0 AND 29)
) STRICT;

-- btech_units.rs and btech_unit_rows.rs: Rust-owned unit state, versioned independently of
-- deferred tables. `unit` holds the rarely changing core and `live` the frequently
-- changing part; together they form one record. Values that count once per simulation
-- second are stored in btech_unit_timers and read as zero in the parts.
CREATE TABLE btech_units (
    dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    state_version INTEGER NOT NULL CHECK (state_version = 1),
    unit TEXT NOT NULL,
    live TEXT NOT NULL DEFAULT '{}'
) STRICT;
-- One row per counter of a unit record (see src/btech/timers.rs). `timer` is a
-- BattleTimer code and `slot` its weapon index, section slot or queue position, else zero.
-- `motion` is 0 (held), 1 (counting down), 2 (counting up) or 3 (wrapping); `anchor` is
-- the held value, the simulation second a countdown reaches zero, or the second a count
-- was zero, taken modulo the cycle when wrapping.
CREATE TABLE btech_unit_timers (
    dbref INTEGER NOT NULL REFERENCES btech_units(dbref) ON DELETE CASCADE,
    timer INTEGER NOT NULL CHECK (timer > 0),
    slot INTEGER NOT NULL CHECK (slot >= 0),
    motion INTEGER NOT NULL CHECK (motion BETWEEN 0 AND 3),
    anchor INTEGER NOT NULL,
    PRIMARY KEY (dbref, timer, slot)
) STRICT, WITHOUT ROWID;

-- btech_vehicles.rs and btech_unit_rows.rs: owned Rust ground-vehicle state, committed with
-- its MUX object and registration, in the same core, live and timer form as btech_units.
CREATE TABLE btech_vehicles (
    dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    state_version INTEGER NOT NULL CHECK (state_version = 1),
    unit TEXT NOT NULL,
    live TEXT NOT NULL DEFAULT '{}'
) STRICT;
CREATE TABLE btech_vehicle_timers (
    dbref INTEGER NOT NULL REFERENCES btech_vehicles(dbref) ON DELETE CASCADE,
    timer INTEGER NOT NULL CHECK (timer > 0),
    slot INTEGER NOT NULL CHECK (slot >= 0),
    motion INTEGER NOT NULL CHECK (motion BETWEEN 0 AND 3),
    anchor INTEGER NOT NULL,
    PRIMARY KEY (dbref, timer, slot)
) STRICT, WITHOUT ROWID;

-- btech_terrain.rs.
-- Per-map encoding version and code dictionary. Maps without a header remain ambiguous.
-- Each code names one distinct hex, stored as the JSON of its layers.
CREATE TABLE btech_map_terrain (
    map_dbref INTEGER PRIMARY KEY REFERENCES btech_maps(dbref) ON DELETE CASCADE,
    encoding_version INTEGER NOT NULL CHECK (encoding_version = 2)
);
CREATE TABLE btech_map_terrain_codes (
    map_dbref INTEGER NOT NULL REFERENCES btech_map_terrain(map_dbref) ON DELETE CASCADE,
    code INTEGER NOT NULL CHECK (code BETWEEN 0 AND 65535),
    hex TEXT NOT NULL CHECK (json_valid(hex)),
    PRIMARY KEY (map_dbref, code),
    UNIQUE (map_dbref, hex)
) WITHOUT ROWID;

-- btech_object_order.rs: explicit traversal order of a map's mines and landing exclusions.
CREATE TABLE btech_mine_order (
    map_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
    PRIMARY KEY (map_dbref, position)
) STRICT, WITHOUT ROWID;
CREATE TABLE btech_landing_order (
    map_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
    PRIMARY KEY (map_dbref, position)
) STRICT, WITHOUT ROWID;

-- btech_decorations.rs.
-- Owned terrain markers; source terrain remains in the map dictionary.
-- A running countdown is stored as the simulation second it ends: `expires_at` for smoke
-- expiry or fire burnout, `spreads_at` for a fire's next spread check. `remaining` holds
-- a value that is not counting down: zero for a permanent marker, or a fire's budget
-- while a spread is pending. Exactly one of `remaining` and `expires_at` is set.
CREATE TABLE btech_map_decorations (
    map_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    tile INTEGER NOT NULL CHECK (tile >= 0 AND tile < 1000000),
    kind TEXT NOT NULL CHECK (kind IN ('fire', 'smoke')),
    remaining INTEGER CHECK (remaining >= -32768 AND remaining <= 4294967295),
    expires_at INTEGER CHECK (expires_at > 0),
    object_duration INTEGER NOT NULL CHECK (object_duration BETWEEN -32768 AND 32767),
    creation_order INTEGER NOT NULL CHECK (creation_order < 0),
    spreads_at INTEGER CHECK (spreads_at > 0),
    PRIMARY KEY (map_dbref, tile),
    CHECK ((remaining IS NULL) <> (expires_at IS NULL))
) STRICT;

-- btech_points_of_interest.rs.
-- Scripted points of interest from a map file, in file order. `type` is case-sensitive.
-- `elevation` is in levels relative to the hex's ground level, or NULL when unset.
CREATE TABLE btech_map_points_of_interest (
    map_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    type TEXT NOT NULL CHECK (length(type) > 0),
    name TEXT NOT NULL CHECK (length(name) > 0),
    x INTEGER NOT NULL CHECK (x BETWEEN 0 AND 999),
    y INTEGER NOT NULL CHECK (y BETWEEN 0 AND 999),
    elevation INTEGER CHECK (elevation BETWEEN -128 AND 127),
    PRIMARY KEY (map_dbref, position)
) STRICT, WITHOUT ROWID;

-- btech_map_random.rs.
-- Map-owned random stream for autonomous fire events, stored as typed generator state.
CREATE TABLE btech_map_random (
    map_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    dice_seed BLOB NOT NULL CHECK (length(dice_seed) = 32),
    dice_stream INTEGER NOT NULL,
    dice_block INTEGER NOT NULL,
    dice_word INTEGER NOT NULL CHECK (dice_word BETWEEN 0 AND 15)
) STRICT;

-- btech_building_repair.rs.
-- Committed simulation countdown for interior-map construction repair, stored as the
-- simulation second the next repair step happens.
CREATE TABLE btech_building_repair (
    map_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    repairs_at INTEGER NOT NULL CHECK (repairs_at > 0)
) STRICT;

-- btech_artillery.rs.
-- Admitted rounds and remaining committed seconds, owned by their battlefield.
-- One row per shot. The shooter is historical and deliberately not a
-- foreign key: losing it never cancels a shell in flight. `mode` is 0 (standard),
-- 1 (cluster), 2 (smoke) or 3 (mine). `arrives_at` is the simulation second the
-- shell lands, so the row stays unchanged while the flight counts down.
CREATE TABLE btech_artillery (
    map_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    shot_id INTEGER NOT NULL CHECK (shot_id BETWEEN 0 AND 4294967295),
    shooter_dbref INTEGER NOT NULL CHECK (shooter_dbref >= 0),
    origin_x INTEGER NOT NULL CHECK (origin_x BETWEEN 0 AND 999),
    origin_y INTEGER NOT NULL CHECK (origin_y BETWEEN 0 AND 999),
    target_x INTEGER NOT NULL CHECK (target_x BETWEEN 0 AND 999),
    target_y INTEGER NOT NULL CHECK (target_y BETWEEN 0 AND 999),
    weapon_part_id INTEGER NOT NULL,
    mode INTEGER NOT NULL CHECK (mode BETWEEN 0 AND 3),
    hit INTEGER NOT NULL CHECK (hit IN (0, 1)),
    arrives_at INTEGER NOT NULL CHECK (arrives_at > 0),
    PRIMARY KEY (map_dbref, shot_id)
) STRICT, WITHOUT ROWID;

-- btech_tows.rs.
-- External tow ownership is independent of unit anatomy and container contents.
-- World validation enforces disjoint pairs across both columns before saving.
-- Targets may be reassigned within a transaction without transient uniqueness conflicts.
CREATE TABLE btech_tows (
    carrier_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    target_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    CHECK(carrier_dbref <> target_dbref)
);

-- btech_wrecks.rs: shared wreck cleanup countdowns.
CREATE TABLE btech_wrecks (
    dbref INTEGER PRIMARY KEY REFERENCES objects(dbref),
    remaining INTEGER NOT NULL CHECK (remaining BETWEEN 1 AND 10)
);

-- btech_recovery.rs.
-- Player-owned consciousness timers and random streams.
-- `mode` is 0 (ready), 1 (character health) or 2 (tactical injuries, which then
-- carries `tactical_injuries`). `recovers_at` is the simulation second of the next
-- recovery check, or NULL while no check is scheduled.
CREATE TABLE btech_character_recovery (
    player_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    mode INTEGER NOT NULL CHECK (mode BETWEEN 0 AND 2),
    tactical_injuries INTEGER CHECK (tactical_injuries BETWEEN 0 AND 255),
    recovers_at INTEGER CHECK (recovers_at > 0),
    pain_resistance INTEGER NOT NULL CHECK (pain_resistance IN (0, 1)),
    toughness INTEGER NOT NULL CHECK (toughness IN (0, 1)),
    dice_seed BLOB NOT NULL CHECK (length(dice_seed) = 32),
    dice_stream INTEGER NOT NULL,
    dice_block INTEGER NOT NULL,
    dice_word INTEGER NOT NULL CHECK (dice_word BETWEEN 0 AND 15),
    CHECK ((mode = 2) = (tactical_injuries IS NOT NULL)),
    CHECK (mode <> 0 OR recovers_at IS NULL)
) STRICT;

-- btech_reactor.rs: the initial reactor instability window. `closes_at` is the
-- simulation second the startup window closes, or NULL once it has closed.
CREATE TABLE btech_reactor_clock (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    closes_at INTEGER CHECK (closes_at > 0)
) STRICT;

-- btech_autopilot.rs.
-- Rust autopilot controllers, one typed row per controller plus child rows for its
-- orders, patrol waypoints and feedback history. Sensor memory is never stored.
--
-- Codes: fire_mode 0 hold, 1 assigned target, 2 opportunistic. state 0 paused, 1 idle,
-- 2 executing, 3 blocked. Reasons 0-11 and feedback events 0-9 follow the declaration
-- order of AutopilotReason and AutopilotFeedbackEvent. Order kind 0 move, 1 hold,
-- 2 follow, 3 patrol, 4 attack, 5 attack-move; order state 0 queued, 1 running,
-- 2 succeeded, 3 failed, 4 canceled.
CREATE TABLE btech_autopilot_controllers (
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
CREATE TABLE btech_autopilot_controller_orders (
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

CREATE TABLE btech_autopilot_controller_waypoints (
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

CREATE TABLE btech_autopilot_controller_feedback (
    unit_dbref INTEGER NOT NULL REFERENCES btech_autopilot_controllers(unit_dbref) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    simulation_time INTEGER NOT NULL,
    order_id INTEGER CHECK (order_id >= 0),
    event INTEGER NOT NULL CHECK (event BETWEEN 0 AND 9),
    reason INTEGER CHECK (reason BETWEEN 0 AND 11),
    PRIMARY KEY (unit_dbref, sequence)
) STRICT, WITHOUT ROWID;
