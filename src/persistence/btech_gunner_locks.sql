-- Independent station lock settling clocks, owned by the station row. `locks_at` is the
-- simulation second the lock settles.
CREATE TABLE btech_gunner_lock_timers (
    station_dbref INTEGER PRIMARY KEY REFERENCES btech_turrets(dbref) ON DELETE CASCADE,
    locks_at INTEGER NOT NULL CHECK (locks_at > 0)
) STRICT;
