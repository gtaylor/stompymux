/*
 * pcache.h
 */

#pragma once
#include "mux/commands/command_queue.h"
#include "mux/database/db.h"
#include "mux/support/red_black_tree.h"

typedef struct player_cache {
  DbRef player;
  int queue;
  int qmax;
  int cflags;
  struct player_cache *next;
} PCACHE;

typedef struct PlayerCache PlayerCache;
typedef struct ServerConfiguration ServerConfiguration;

enum : int { PF_DEAD = 0x0001, PF_REF = 0x0002, PF_QMAX_CH = 0x0004 };

PlayerCache *player_cache_create(const ServerConfiguration *configuration,
                                 GameDatabase *database);
void player_cache_destroy(PlayerCache *cache);
PCACHE *pcache_find(PlayerCache *cache, DbRef player);
void pcache_reload(PlayerCache *cache, DbRef player);
void pcache_sync(PlayerCache *cache);
void pcache_trim(PlayerCache *cache);
int queue_adjust(PlayerCache *cache, DbRef player, int adj);
void queue_set(PlayerCache *cache, DbRef player, int val);
int queue_maximum(PlayerCache *cache, DbRef player);
