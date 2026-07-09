#ifndef __LOGCACHE_H__
#define __LOGCACHE_H__

void logcache_list(dbref player);
#ifndef BTMUX_LOGCACHE_H
#define BTMUX_LOGCACHE_H

#include "db.h"

void logcache_list(dbref player);
void logcache_init(void);
void logcache_destruct(void);
int logcache_writelog(char *fname, char *fdata);

#endif
int logcache_writelog(char *fname, char *fdata);

#endif
