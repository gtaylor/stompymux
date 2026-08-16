#include <stdbool.h>

#include "btech/context.h"
#include "context_internal.h"
#include "missile_hit_registry.h"

static MissileHitEntry fixture_entry = {
    .name = "fixture",
    .weapon_index = 42,
    .num_missiles = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11},
};

const MissileHitEntry *missile_hit_registry_find_weapon(
    const MissileHitRegistry *registry [[maybe_unused]], int weapon_index) {
  return weapon_index == fixture_entry.weapon_index ? &fixture_entry : nullptr;
}

const MissileHitEntry *missile_hit_registry_find_name(
    const MissileHitRegistry *registry [[maybe_unused]], const char *name) {
  return name != nullptr && name[0] == 'f' ? &fixture_entry : nullptr;
}

static int test_roll_boundaries(void) {
  BtechContext context = {0};
  context.missile_hits = (MissileHitRegistry){0};

  if (btech_context_missile_hit_count(&(MissileHitLookup){
          .context = &context, .weapon = 42, .roll = -1}) != 1)
    return 1;
  if (btech_context_missile_hit_count(&(MissileHitLookup){
          .context = &context, .weapon = 42, .roll = 10}) != 11)
    return 1;
  if (btech_context_missile_hit_count(&(MissileHitLookup){
          .context = &context, .weapon = 42, .roll = 11}) != 11)
    return 1;
  if (btech_context_missile_hit_count_by_name(&context, "fixture", -1) != 1)
    return 1;
  return btech_context_missile_hit_count_by_name(&context, "fixture", 11) != 11;
}

int main(void) { return test_roll_boundaries(); }
