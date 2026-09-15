-- Rust-owned unit construction and runtime state, versioned independently of deferred tables.
CREATE TABLE btech_units (
    dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    state_version INTEGER NOT NULL CHECK (state_version = 1),
    unit TEXT NOT NULL
);
