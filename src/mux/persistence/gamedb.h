/* gamedb.h -- SQLite-backed game-database persistence */

#pragma once

/*
 * Persist the complete current game state to the configured SQLite database.
 * The implementation writes a temporary database and atomically replaces the
 * target, so a failed dump leaves the prior database available. Returns 0 on
 * success and -1 after logging a failure.
 */
int gamedb_dump(int dump_type);

/*
 * Rebuild the in-memory game database from a SQLite file. The caller must
 * initialize the attribute and hash-table subsystems first. Returns 0 on
 * success and -1 after logging a validation or SQLite error.
 */
int gamedb_load(const char *path);
