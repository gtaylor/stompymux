-- Committed simulation countdown for interior-map construction repair.
CREATE TABLE btech_building_repair (
    map_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    remaining INTEGER NOT NULL CHECK(remaining BETWEEN 1 AND 120)
);
