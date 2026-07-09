/*
 * pcache.h
 */

#pragma once
#include "db.h"
#include "cque.h"
#include "rbtree.h"

typedef struct player_cache {
    dbref player;
    int money;
    int queue;
    int qmax;
    int cflags;
    struct player_cache *next;
} PCACHE;

#define PF_DEAD     0x0001
#define PF_REF      0x0002
#define PF_MONEY_CH 0x0004
#define PF_QMAX_CH  0x0008

void pcache_init(void);
PCACHE *pcache_find(dbref player);
void pcache_reload(dbref player);
void pcache_sync(void);
void pcache_trim(void);
int a_Queue(dbref player, int adj);
void s_Queue(dbref player, int val);
int QueueMax(dbref player);
