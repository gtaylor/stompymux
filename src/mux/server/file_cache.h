/* file_cache.h - Cached text-file loading and display declarations. */

#pragma once

#include "mux/database/db.h"
#include "mux/network/descriptor.h"

/* File caches. These _must_ track the fcache array in file_cache.c. */

#define FC_CONN 0
#define FC_CONN_SITE 1
#define FC_CONN_DOWN 2
#define FC_CONN_FULL 3
#define FC_CONN_REG 4
#define FC_CREA_NEW 5
#define FC_CREA_REG 6
#define FC_QUIT 7
#define FC_LAST 7

/* File cache routines */

extern void fcache_rawdump(int fd, int num);
extern void fcache_dump(Descriptor *d, int num);
extern void fcache_dump_conn(Descriptor *d, int num);
extern void fcache_send(DbRef, int);
extern void fcache_load(DbRef);
extern void fcache_init(void);
