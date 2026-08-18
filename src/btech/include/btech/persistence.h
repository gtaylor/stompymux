/** @file
 * BTech data stored with the MUX SQLite game database.
 */

#pragma once

typedef struct PersistenceContext PersistenceContext;
typedef struct BtechContext BtechContext;

/**
 * Registers BTech's optional SQLite persistence extension.
 *
 * This must happen during startup before the MUX game database is loaded or
 * dumped.
 *
 * @param[in,out] context Persistence registry to extend.
 * @param[in,out] btech BTech runtime context used by the extension.
 * @return Zero on success; otherwise an error code.
 */
int btech_persistence_register(PersistenceContext *context,
                               BtechContext *btech);

/**
 * Loads persisted BTech special-object state from a game database.
 *
 * Call after `LoadSpecialObjects()` has constructed
 * the normal BTech registries and object instances. This is intentionally
 * separate from core `gamedb_load()`, which runs before those objects exist.
 *
 * @param[in,out] context BTech runtime context receiving loaded state.
 * @param[in] path Path to the game database.
 * @return Zero on success; otherwise an error code.
 */
int btech_persistence_load_special_state_path(BtechContext *context,
                                              const char *path);

/**
 * Removes every BTech-owned table from an offline game database.
 *
 * The next successful dump creates the current schema. Callers must ensure the
 * game server is stopped before invoking this operation.
 *
 * @param[in] path Path to the offline game database.
 * @return Zero on success; otherwise an error code.
 */
int btech_persistence_reset_schema_path(const char *path);
