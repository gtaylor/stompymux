-- External tow ownership is independent of unit anatomy and container contents.
-- World validation enforces disjoint pairs across both columns before saving.
-- Targets may be reassigned within a transaction without transient uniqueness conflicts.
CREATE TABLE btech_tows (
    carrier_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    target_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    CHECK(carrier_dbref <> target_dbref)
);
