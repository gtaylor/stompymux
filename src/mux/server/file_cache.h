/** @file
 * Cached text-file loading and display declarations.
 */
#pragma once

#include "mux/lua/lua_runtime.h"
#include "mux/server/game.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

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

/** Creates file cache. @param[in] evaluation Expression evaluation context.
 * @param[in] configuration Server configuration. @param[in] descriptors
 * Descriptors. */

FileCache *file_cache_create(EvaluationContext *evaluation,
                             const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors);
/** Destroys file cache. @param[in,out] cache Cache. */

void file_cache_destroy(FileCache *cache);
typedef struct FileCacheRawDumpRequest {
  const FileCache *cache;
  int descriptor;
  int entry;
} FileCacheRawDumpRequest;

/** Executes fcache rawdump. @param[in] request Request. */

void fcache_rawdump(const FileCacheRawDumpRequest *request);
/** Executes fcache dump. @param[in] cache Cache. @param[in,out] descriptor
 * Network descriptor. @param[in] num Num. */

void fcache_dump(const FileCache *cache, Descriptor *descriptor, int num);
/** Executes fcache dump conn. @param[in] cache Cache. @param[in,out] descriptor
 * Network descriptor. @param[in] num Num. */

void fcache_dump_conn(const FileCache *cache, Descriptor *descriptor, int num);
/** Executes fcache load. @param[in,out] evaluation Expression evaluation
 * context. @param[in,out] cache Cache. @param[in] player Player object. */

void fcache_load(EvaluationContext *evaluation, FileCache *cache, DbRef player);
/** Counts file cache connection. @param[in] cache Cache. */

int file_cache_connection_count(const FileCache *cache);
