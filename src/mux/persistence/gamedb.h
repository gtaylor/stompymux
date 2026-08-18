/** @file
 * - SQLite-backed game-database persistence.
 */
#pragma once

#include <sqlite3.h>
#include <stddef.h>
#include <time.h>

typedef struct ChannelRegistry ChannelRegistry;
typedef struct GameDatabase GameDatabase;
typedef struct MacroRegistry MacroRegistry;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;
typedef struct WorldContext WorldContext;

typedef struct PersistenceContext PersistenceContext;

/*
 * A persistence extension stores and restores subsystem state in the same
 * SQLite transaction and database file as the core MUX game state.
 */
typedef int (*PersistenceSqliteLoad)(sqlite3 *sqlite,
                                     PersistenceContext *context,
                                     void *extension_context);
typedef int (*PersistenceSqliteStore)(sqlite3 *sqlite,
                                      PersistenceContext *context,
                                      void *extension_context);

constexpr int PERSISTENCE_MAX_SQLITE_EXTENSIONS = 8;

typedef struct PersistenceSqliteExtension PersistenceSqliteExtension;
struct PersistenceSqliteExtension {
  /* Registration metadata and callback context are borrowed. */
  const char *name;
  PersistenceSqliteLoad load;
  PersistenceSqliteStore store;
  void *context;
};

struct PersistenceContext {
  /* Runtime service pointers are borrowed from MuxServer. */
  const ServerConfiguration *configuration;
  GameDatabase *database;
  ChannelRegistry *channels;
  MacroRegistry *macros;
  time_t *now;
  int *record_players;
  WorldContext *world;
  ServerLog *log;

  /* The persistence context owns its bounded extension registry. */
  PersistenceSqliteExtension extensions[PERSISTENCE_MAX_SQLITE_EXTENSIONS];
  size_t extension_count;
};

/** Initializes persistence context. @param[out] context Operation context.
 * @param[in] configuration Server configuration. @param[in] database Game
 * database. @param[in] channels Channels. @param[in] macros Macros. @param[in]
 * now Now. @param[in] record_players Record players. @param[in] world World.
 * @param[in] log Server log. */

void persistence_context_initialize(
    PersistenceContext *context, const ServerConfiguration *configuration,
    GameDatabase *database, ChannelRegistry *channels, MacroRegistry *macros,
    time_t *now, int *record_players, WorldContext *world, ServerLog *log);
/** Returns persistence extension at. @param[in] context Operation context.
 * @param[in] index Zero-based index. */

PersistenceSqliteExtension *
persistence_extension_at(PersistenceContext *context, size_t index);

/*
 * Register a named SQLite extension before gamedb_load() or gamedb_dump().
 * The callbacks are invoked with the SQLite connection owned by this module;
 * store callbacks run inside the snapshot transaction. A load callback may be
 * nullptr when the subsystem restores its data after core loading. Returns 0
 * on success and -1 if the registration is invalid or the extension limit is
 * reached.
 */
/** Executes persistence register sqlite extension. @param[in,out] context
 * Operation context. @param[in] name Name to use. @param[in] load Load.
 * @param[in] store Store. @param[in,out] extension_context Extension context.
 */

int persistence_register_sqlite_extension(PersistenceContext *context,
                                          const char *name,
                                          PersistenceSqliteLoad load,
                                          PersistenceSqliteStore store,
                                          void *extension_context);

/*
 * Persist the complete current game state to the configured SQLite database.
 * The implementation writes a temporary database and atomically replaces the
 * target, so a failed dump leaves the prior database available. Returns 0 on
 * success and -1 after logging a failure.
 */
/** Executes gamedb dump. @param[in,out] context Operation context. @param[in]
 * dump_type Dump type. */

int gamedb_dump(PersistenceContext *context, int dump_type);

/* Persist a new normal snapshot without replacing an existing path. */
/** Creates gamedb. @param[in] context Operation context. */

int gamedb_create(PersistenceContext *context);

/*
 * Rebuild the in-memory game database from a SQLite file. The caller must
 * initialize the attribute and hash-table subsystems first. Returns 0 on
 * success and -1 after logging a validation or SQLite error.
 */
/** Executes gamedb load. @param[in,out] context Operation context. @param[in]
 * path Filesystem path. */

int gamedb_load(PersistenceContext *context, const char *path);
