/** @file
 * pcache.h.
 */
#pragma once
#include <stddef.h>

#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

typedef struct PlayerCache PlayerCache;
typedef struct ServerConfiguration ServerConfiguration;

/** Creates player cache. @param[in] configuration Server configuration.
 * @param[in] database Game database. */

PlayerCache *player_cache_create(const ServerConfiguration *configuration,
                                 GameDatabase *database);
/** Destroys player cache. @param[in,out] cache Cache. */

void player_cache_destroy(PlayerCache *cache);
/** Executes player cache size. @param[in] cache Cache. */

size_t player_cache_size(const PlayerCache *cache);
/** Executes player cache trim. @param[in,out] cache Cache. */

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

/** Executes queue adjust. @param[in] adjustment Adjustment. */

int queue_adjust(const PlayerQueueAdjustment *adjustment);
/** Sets queue. @param[in] assignment Assignment. */

void queue_set(const PlayerQueueAssignment *assignment);
/** Executes queue maximum. @param[in,out] cache Cache. @param[in] player Player
 * object. */

int queue_maximum(PlayerCache *cache, DbRef player);
