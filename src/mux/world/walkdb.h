/* walkdb.h - Database-wide search, statistics, and ownership helper interface.
 */

#pragma once

#include "mux/database/db.h"

int chown_all(DbRef from_player, DbRef to_player);
void olist_push(void);
void olist_pop(void);
void olist_add(DbRef object);
DbRef olist_first(void);
DbRef olist_next(void);
