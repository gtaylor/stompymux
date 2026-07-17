/* file_cache.h - Cached text-file loading and display declarations. */

#pragma once

#include "mux/database/db.h"
#include "mux/network/descriptor.h"

/* File caches. These _must_ track the fcache array in file_cache.c. */

enum : int {
  FC_CONN = 0,
  FC_CONN_SITE = 1,
  FC_CONN_DOWN = 2,
  FC_CONN_FULL = 3,
  FC_QUIT = 4,
  FC_LAST = FC_QUIT,
};

/* File cache routines */

extern void fcache_rawdump(int fd, int num);
extern void fcache_dump(Descriptor *d, int num);
extern void fcache_dump_conn(Descriptor *d, int num);
extern void fcache_send(DbRef, int);
extern void fcache_load(DbRef);
extern void fcache_init(void);
