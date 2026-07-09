#pragma once

#include "db.h"

void logcache_list(dbref player);
void logcache_init(void);
void logcache_destruct(void);
int logcache_writelog(char *fname, char *fdata);
