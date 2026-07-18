/* gamedb.h -- SQLite-backed game-database persistence */

#pragma once

#include <sqlite3.h>
#include <stddef.h>
#include <time.h>

typedef struct ChannelRegistry ChannelRegistry;
typedef struct GameDatabase GameDatabase;
typedef struct MacroRegistry MacroRegistry;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;
typedef struct VattrStore VattrStore;
typedef struct WorldContext WorldContext;

typedef struct PersistenceContext PersistenceContext;

/*
 * A persistence extension stores and restores subsystem state in the same
 * SQLite transaction and database file as the core MUX game state.
 */
typedef int (*PersistenceSqliteLoad)(sqlite3 *sqlite,
                                     PersistenceContext *context);
typedef int (*PersistenceSqliteStore)(sqlite3 *sqlite,
                                      PersistenceContext *context);

constexpr int PERSISTENCE_MAX_SQLITE_EXTENSIONS = 8;

typedef struct PersistenceSqliteExtension PersistenceSqliteExtension;
struct PersistenceSqliteExtension {
  const char *name;
  PersistenceSqliteLoad load;
  PersistenceSqliteStore store;
};

struct PersistenceContext {
  const ServerConfiguration *configuration;
  GameDatabase *database;
  VattrStore *vattrs;
  ChannelRegistry *channels;
  MacroRegistry *macros;
  time_t *now;
  int *record_players;
  WorldContext *world;
  ServerLog *log;
  PersistenceSqliteExtension extensions[PERSISTENCE_MAX_SQLITE_EXTENSIONS];
  size_t extension_count;
};

void persistence_context_initialize(PersistenceContext *context,
                                    const ServerConfiguration *configuration,
                                    GameDatabase *database, VattrStore *vattrs,
                                    ChannelRegistry *channels,
                                    MacroRegistry *macros, time_t *now,
                                    int *record_players, WorldContext *world,
                                    ServerLog *log);

/*
 * Register a named SQLite extension before gamedb_load() or gamedb_dump().
 * The callbacks are invoked with the SQLite connection owned by this module;
 * store callbacks run inside the snapshot transaction. Returns 0 on success
 * and -1 if the registration is invalid or the extension limit is reached.
 */
int persistence_register_sqlite_extension(PersistenceContext *context,
                                          const char *name,
                                          PersistenceSqliteLoad load,
                                          PersistenceSqliteStore store);

/*
 * Persist the complete current game state to the configured SQLite database.
 * The implementation writes a temporary database and atomically replaces the
 * target, so a failed dump leaves the prior database available. Returns 0 on
 * success and -1 after logging a failure.
 */
int gamedb_dump(PersistenceContext *context, int dump_type);

/*
 * Rebuild the in-memory game database from a SQLite file. The caller must
 * initialize the attribute and hash-table subsystems first. Returns 0 on
 * success and -1 after logging a validation or SQLite error.
 */
int gamedb_load(PersistenceContext *context, const char *path);
