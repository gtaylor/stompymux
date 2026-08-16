/*
 * pcache.h
 */

#pragma once
#include <stddef.h>

#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

typedef struct PlayerCache PlayerCache;
typedef struct ServerConfiguration ServerConfiguration;

PlayerCache *player_cache_create(const ServerConfiguration *configuration,
                                 GameDatabase *database);
void player_cache_destroy(PlayerCache *cache);
size_t player_cache_size(const PlayerCache *cache);
void player_cache_trim(PlayerCache *cache);
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
