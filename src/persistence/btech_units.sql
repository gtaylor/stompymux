-- Rust-owned unit state, versioned independently of deferred tables. `unit` holds the
-- rarely changing core and `live` the frequently changing part; together they form one
-- record. `clocks` stores values that count once per simulation second as clock forms
-- (see btech_clocks.rs); their places in `unit` and `live` hold a placeholder.
CREATE TABLE btech_units (
    dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    state_version INTEGER NOT NULL CHECK (state_version = 1),
    unit TEXT NOT NULL,
    live TEXT NOT NULL DEFAULT '{}',
    clocks TEXT NOT NULL DEFAULT '{}'
) STRICT;
