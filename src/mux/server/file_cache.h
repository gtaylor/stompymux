/* file_cache.h - Cached text-file loading and display declarations. */

#pragma once

#include "mux/network/descriptor.h"
#include "mux/objects/db.h"

typedef struct FileCache FileCache;
typedef struct ServerConfiguration ServerConfiguration;

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

FileCache *file_cache_create(EvaluationContext *evaluation,
                             const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors);
void file_cache_destroy(FileCache *cache);
void fcache_rawdump(const FileCache *cache, int fd, int num);
void fcache_dump(const FileCache *cache, Descriptor *descriptor, int num);
void fcache_dump_conn(const FileCache *cache, Descriptor *descriptor, int num);
void fcache_send(FileCache *cache, DbRef player, int num);
void fcache_load(EvaluationContext *evaluation, FileCache *cache, DbRef player);
int file_cache_connection_count(const FileCache *cache);
