BEGIN IMMEDIATE;

CREATE TABLE btech_mech_positions_v3 (
  mech_dbref INTEGER PRIMARY KEY,
  pilot_status INTEGER NOT NULL,
  hexes_walked REAL NOT NULL,
  facing INTEGER NOT NULL,
  x INTEGER NOT NULL,
  y INTEGER NOT NULL,
  z INTEGER NOT NULL,
  last_x INTEGER NOT NULL,
  last_y INTEGER NOT NULL,
  fx REAL NOT NULL,
  fy REAL NOT NULL,
  fz REAL NOT NULL,
  team INTEGER NOT NULL,
  unusable_arcs INTEGER NOT NULL,
  stall INTEGER NOT NULL,
  pilot INTEGER NOT NULL
);

INSERT INTO btech_mech_positions_v3
  (mech_dbref, pilot_status, hexes_walked, facing, x, y, z, last_x, last_y,
   fx, fy, fz, team, unusable_arcs, stall, pilot)
SELECT mech_dbref, pilot_status, hexes_walked, facing, x, y, z, last_x, last_y,
       fx, fy, fz, team, unusable_arcs, stall, pilot
FROM btech_mech_positions;

DROP TABLE btech_mech_positions;
ALTER TABLE btech_mech_positions_v3 RENAME TO btech_mech_positions;

CREATE TABLE btech_persistence_metadata_v3 (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  schema_name TEXT NOT NULL CHECK (schema_name = 'stompymux-btech'),
  schema_version INTEGER NOT NULL CHECK (schema_version = 3)
);

INSERT INTO btech_persistence_metadata_v3 (id, schema_name, schema_version)
SELECT id, schema_name, 3
FROM btech_persistence_metadata;

DROP TABLE btech_persistence_metadata;
ALTER TABLE btech_persistence_metadata_v3 RENAME TO btech_persistence_metadata;

COMMIT;
