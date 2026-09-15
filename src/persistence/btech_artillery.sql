-- Admitted rounds and remaining committed seconds, owned by their battlefield.
CREATE TABLE btech_artillery (
    map_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE,
    shots TEXT NOT NULL CHECK(length(CAST(shots AS BLOB)) <= 1048576)
);
