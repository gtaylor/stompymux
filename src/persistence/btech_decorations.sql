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
