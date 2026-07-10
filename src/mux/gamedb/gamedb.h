/* gamedb.h -- SQLite game database snapshot support */

#pragma once

/*
 * Write a SQLite mirror of the currently loaded game database.
 */
int gamedb_dump(int dump_type);

/* Load the in-memory game database from a SQLite snapshot. */
int gamedb_load(const char *path);
