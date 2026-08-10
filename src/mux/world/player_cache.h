/*
 * pcache.h
 */

#pragma once
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

typedef struct player_cache {
  DbRef player;
  int queue;
  int cflags;
  struct player_cache *next;
} PCACHE;

typedef struct PlayerCache PlayerCache;
typedef struct ServerConfiguration ServerConfiguration;

enum : int { PF_DEAD = 0x0001, PF_REF = 0x0002 };

PlayerCache *player_cache_create(const ServerConfiguration *configuration,
                                 GameDatabase *database);
void player_cache_destroy(PlayerCache *cache);
PCACHE *pcache_find(PlayerCache *cache, DbRef player);
void pcache_trim(PlayerCache *cache);
typedef struct PlayerQueueAdjustment {
  PlayerCache *cache;
  DbRef player;
  int delta;
} PlayerQueueAdjustment;

typedef struct PlayerQueueAssignment {
  PlayerCache *cache;
  DbRef player;
  int value;
} PlayerQueueAssignment;

int queue_adjust(const PlayerQueueAdjustment *adjustment);
void queue_set(const PlayerQueueAssignment *assignment);
int queue_maximum(PlayerCache *cache, DbRef player);
