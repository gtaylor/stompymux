-- Map-owned random stream for autonomous fire events.
CREATE TABLE btech_map_random (
    map_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    dice TEXT NOT NULL
);
