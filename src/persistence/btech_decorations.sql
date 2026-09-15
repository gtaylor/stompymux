-- Owned terrain markers; zero lifetime means permanent; source terrain remains in the map dictionary.
CREATE TABLE btech_map_decorations (
    map_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    tile INTEGER NOT NULL CHECK(tile >= 0 AND tile < 1000000),
    kind TEXT NOT NULL CHECK(kind IN ('fire','smoke')),
    remaining INTEGER NOT NULL CHECK(remaining >= -32768 AND remaining <= 4294967295),
    object_duration INTEGER NOT NULL CHECK(object_duration BETWEEN -32768 AND 32767),
    creation_order INTEGER NOT NULL CHECK(creation_order < 0),
    next_spread INTEGER CHECK(next_spread > 0 AND next_spread <= 60),
    PRIMARY KEY(map_dbref,tile)
);
