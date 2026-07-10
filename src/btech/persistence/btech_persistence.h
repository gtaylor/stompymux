/* btech_persistence.h -- BTech data stored with the MUX SQLite game database */

#pragma once

/*
 * Register BTech's optional SQLite persistence extension. This must happen
 * during startup before the MUX game database is loaded or dumped.
 */
int btech_persistence_register(void);
