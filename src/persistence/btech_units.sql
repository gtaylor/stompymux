-- Rust-owned unit state, versioned independently of deferred tables. `unit` holds the
-- rarely changing core and `live` the per-tick state; together they form one record.
CREATE TABLE btech_units (
    dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    state_version INTEGER NOT NULL CHECK (state_version = 1),
    unit TEXT NOT NULL,
    live TEXT NOT NULL DEFAULT '{}'
);
