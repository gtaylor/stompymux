/* log_cache.h - Cached arbitrary-log file management interface. */

#pragma once

#include "mux/database/db.h"

void logcache_list(DbRef player);
void logcache_init(void);
void logcache_destruct(void);
int logcache_writelog(char *fname, char *fdata);
