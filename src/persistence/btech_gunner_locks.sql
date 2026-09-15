-- Independent station lock settling clocks, owned by the station row.
CREATE TABLE btech_gunner_lock_timers (
    station_dbref INTEGER PRIMARY KEY REFERENCES btech_turrets(dbref) ON DELETE CASCADE,
    remaining INTEGER NOT NULL CHECK (remaining BETWEEN 1 AND 8)
);
