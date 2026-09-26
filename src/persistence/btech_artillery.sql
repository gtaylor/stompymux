-- Admitted rounds and remaining committed seconds, owned by their battlefield.
-- One row per shot. The shooter and station are historical and deliberately not
-- foreign keys: losing either never cancels a shell in flight. `mode` is 0 (standard),
-- 1 (cluster), 2 (smoke) or 3 (mine).
CREATE TABLE btech_artillery (
    map_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    shot_id INTEGER NOT NULL CHECK (shot_id BETWEEN 0 AND 4294967295),
    shooter_dbref INTEGER NOT NULL CHECK (shooter_dbref >= 0),
    station_dbref INTEGER CHECK (station_dbref >= 0),
    origin_x INTEGER NOT NULL CHECK (origin_x BETWEEN 0 AND 999),
    origin_y INTEGER NOT NULL CHECK (origin_y BETWEEN 0 AND 999),
    target_x INTEGER NOT NULL CHECK (target_x BETWEEN 0 AND 999),
    target_y INTEGER NOT NULL CHECK (target_y BETWEEN 0 AND 999),
    weapon_part_id INTEGER NOT NULL,
    mode INTEGER NOT NULL CHECK (mode BETWEEN 0 AND 3),
    hit INTEGER NOT NULL CHECK (hit IN (0, 1)),
    remaining INTEGER NOT NULL CHECK (remaining BETWEEN 1 AND 65535),
    PRIMARY KEY (map_dbref, shot_id)
) STRICT, WITHOUT ROWID;
