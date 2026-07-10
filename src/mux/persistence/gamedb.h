/* gamedb.h -- SQLite-backed game-database persistence */

#pragma once

#include <sqlite3.h>

/*
 * A persistence extension stores and restores subsystem state in the same
 * SQLite transaction and database file as the core MUX game state.
 */
typedef int (*PERSISTENCE_SQLITE_LOAD)(sqlite3 *sqlite);
typedef int (*PERSISTENCE_SQLITE_STORE)(sqlite3 *sqlite);

/*
 * Register a named SQLite extension before gamedb_load() or gamedb_dump().
 * The callbacks are invoked with the SQLite connection owned by this module;
 * store callbacks run inside the snapshot transaction. Returns 0 on success
 * and -1 if the registration is invalid or the extension limit is reached.
 */
int persistence_register_sqlite_extension(const char *name,
                                          PERSISTENCE_SQLITE_LOAD load,
                                          PERSISTENCE_SQLITE_STORE store);

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
