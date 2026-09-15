-- Nonzero trajectory corrections belong to station targeting, independently of its parent.
CREATE TABLE btech_gunner_artillery (
    station_dbref INTEGER PRIMARY KEY REFERENCES btech_turrets(dbref) ON DELETE CASCADE,
    adjustment INTEGER NOT NULL CHECK(adjustment BETWEEN 1 AND 255)
);
