-- Player-owned consciousness timers and random streams.
CREATE TABLE btech_character_recovery (
    player_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    state_version INTEGER NOT NULL CHECK(state_version = 1),
    recovery TEXT NOT NULL
);
