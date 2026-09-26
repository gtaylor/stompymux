-- Map-owned random stream for autonomous fire events, stored as typed generator state.
CREATE TABLE btech_map_random (
    map_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    dice_seed BLOB NOT NULL CHECK (length(dice_seed) = 32),
    dice_stream INTEGER NOT NULL,
    dice_block INTEGER NOT NULL,
    dice_word INTEGER NOT NULL CHECK (dice_word BETWEEN 0 AND 15)
) STRICT;
