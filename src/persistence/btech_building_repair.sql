-- Committed simulation countdown for interior-map construction repair, stored as the
-- simulation second the next repair step happens.
CREATE TABLE btech_building_repair (
    map_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    repairs_at INTEGER NOT NULL CHECK (repairs_at > 0)
) STRICT;
