-- Offline migration for BTech persistence schema versions 6 and 7.
-- Stop the server and back up the database before running this script.
-- sqlite3 game/data/stompymux.db < game/data/migrations/btech-persistence-v8.sql

.bail on
BEGIN IMMEDIATE;

CREATE TEMP TABLE btech_v8_migration_guard (
  valid INTEGER NOT NULL CHECK (valid = 1)
);
INSERT INTO btech_v8_migration_guard
SELECT CASE WHEN count(*) = 1 THEN 1 ELSE 0 END
FROM btech_persistence_metadata
WHERE id = 1 AND schema_name = 'stompymux-btech'
  AND schema_version IN (6, 7);

ALTER TABLE btech_autopilots
  ADD COLUMN engaged INTEGER NOT NULL DEFAULT 0 CHECK (engaged IN (0, 1));

UPDATE btech_autopilots AS autopilot
SET engaged = CASE
  WHEN (autopilot.flags & 7) != 0 THEN 1
  WHEN EXISTS (
    SELECT 1
    FROM objects AS object
    JOIN btech_mech_runtime AS runtime
      ON runtime.mech_dbref = autopilot.mech_dbref
    WHERE object.dbref = autopilot.dbref
      AND object.location = autopilot.mech_dbref
      AND runtime.autopilot_num = autopilot.dbref
      AND EXISTS (
        SELECT 1
        FROM btech_autopilot_commands AS command
        WHERE command.autopilot_dbref = autopilot.dbref
      )
  ) THEN 1
  ELSE 0
END;

ALTER TABLE btech_persistence_metadata RENAME TO btech_persistence_metadata_v7;
CREATE TABLE btech_persistence_metadata (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  schema_name TEXT NOT NULL CHECK (schema_name = 'stompymux-btech'),
  schema_version INTEGER NOT NULL CHECK (schema_version = 8)
);
INSERT INTO btech_persistence_metadata (id, schema_name, schema_version)
VALUES (1, 'stompymux-btech', 8);
DROP TABLE btech_persistence_metadata_v7;

DROP TABLE btech_v8_migration_guard;
COMMIT;
