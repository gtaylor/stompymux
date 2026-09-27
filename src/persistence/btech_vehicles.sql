-- Owned Rust ground-vehicle state, committed with its MUX object and registration.
-- `unit` holds the rarely changing core and `live` the frequently changing part.
-- `clocks` stores values that count once per simulation second as clock forms (see
-- btech_clocks.rs); their places in `unit` and `live` hold a placeholder.
CREATE TABLE btech_vehicles (
    dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    state_version INTEGER NOT NULL CHECK (state_version = 1),
    unit TEXT NOT NULL,
    live TEXT NOT NULL DEFAULT '{}',
    clocks TEXT NOT NULL DEFAULT '{}'
) STRICT;
