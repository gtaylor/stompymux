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

enum : int { PF_DEAD = 0x0001, PF_REF = 0x0002, PF_QMAX_CH = 0x0004 };

void pcache_init(void);
PCACHE *pcache_find(DbRef player);
void pcache_reload(DbRef player);
void pcache_sync(void);
void pcache_trim(void);
int queue_adjust(DbRef player, int adj);
void queue_set(DbRef player, int val);
int queue_maximum(DbRef player);
