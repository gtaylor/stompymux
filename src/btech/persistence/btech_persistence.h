/* btech_persistence.h -- BTech data stored with the MUX SQLite game database */

#pragma once

/*
 * Register BTech's optional SQLite persistence extension. This must happen
 * during startup before the MUX game database is loaded or dumped.
 */
int btech_persistence_register(void);

/*
 * Load BTech special-object state after LoadSpecialObjects() has constructed
 * the normal BTech registries and object instances. This is intentionally
 * separate from core gamedb_load(), which runs before those objects exist.
 */
int btech_persistence_load_special_state_path(const char *path);
