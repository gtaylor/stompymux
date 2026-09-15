-- Owned Rust ground-vehicle state, committed with its MUX object and registration.
CREATE TABLE btech_vehicles (
    dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    state_version INTEGER NOT NULL CHECK (state_version = 1),
    unit TEXT NOT NULL
);
