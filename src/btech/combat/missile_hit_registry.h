#pragma once

#include <stdbool.h>
#include <stddef.h>

typedef struct BtechContext BtechContext;

enum { BTECH_MISSILE_HIT_ROLL_COUNT = 11 };

typedef struct MissileHitEntry {
  const char *name;
  int weapon_index;
  int num_missiles[BTECH_MISSILE_HIT_ROLL_COUNT];
} MissileHitEntry;

typedef struct MissileHitRegistry {
  MissileHitEntry *entries;
  size_t count;
} MissileHitRegistry;

bool missile_hit_registry_initialize(MissileHitRegistry *registry,
                                     BtechContext *context);
void missile_hit_registry_destroy(MissileHitRegistry *registry);
const MissileHitEntry *
missile_hit_registry_find_weapon(const MissileHitRegistry *registry,
                                 int weapon_index);
const MissileHitEntry *
missile_hit_registry_find_name(const MissileHitRegistry *registry,
                               const char *name);
