-- Player-owned consciousness timers and random streams.
-- `mode` is 0 (ready), 1 (character health) or 2 (tactical injuries, which then
-- carries `tactical_injuries`).
CREATE TABLE btech_character_recovery (
    player_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    mode INTEGER NOT NULL CHECK (mode BETWEEN 0 AND 2),
    tactical_injuries INTEGER CHECK (tactical_injuries BETWEEN 0 AND 255),
    remaining INTEGER NOT NULL CHECK (remaining BETWEEN 0 AND 30),
    pain_resistance INTEGER NOT NULL CHECK (pain_resistance IN (0, 1)),
    toughness INTEGER NOT NULL CHECK (toughness IN (0, 1)),
    dice_seed BLOB NOT NULL CHECK (length(dice_seed) = 32),
    dice_stream INTEGER NOT NULL,
    dice_block INTEGER NOT NULL,
    dice_word INTEGER NOT NULL CHECK (dice_word BETWEEN 0 AND 15),
    CHECK ((mode = 2) = (tactical_injuries IS NOT NULL)),
    CHECK (mode <> 0 OR remaining = 0)
) STRICT;
