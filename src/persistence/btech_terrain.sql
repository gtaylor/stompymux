-- Per-map encoding version and code dictionary. Maps without a header remain ambiguous.
CREATE TABLE btech_map_terrain (
    map_dbref INTEGER PRIMARY KEY REFERENCES btech_maps(dbref) ON DELETE CASCADE,
    encoding_version INTEGER NOT NULL CHECK (encoding_version = 1)
);
CREATE TABLE btech_map_terrain_codes (
    map_dbref INTEGER NOT NULL REFERENCES btech_map_terrain(map_dbref) ON DELETE CASCADE,
    code INTEGER NOT NULL CHECK (code BETWEEN 0 AND 255),
    terrain TEXT NOT NULL CHECK (length(terrain) = 1),
    elevation INTEGER NOT NULL CHECK (elevation BETWEEN 0 AND 9),
    PRIMARY KEY (map_dbref, code),
    UNIQUE (map_dbref, terrain, elevation)
) WITHOUT ROWID;
