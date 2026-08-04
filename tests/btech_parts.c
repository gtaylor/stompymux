#include "mech.parts.h"

#include "btech_context.h"
#include "btmacros.h"
#include "btmux_build_config.h"
#include "mech.h"
#include "mux/objects/db.h"

static int inventory_count = 6;
static DbRef changed_store;
static int changed_part;
static int changed_brand;
static int changed_count;

int alias_part(MECH *mech, int part, int location) {
  (void)mech;
  return part + location;
}

int econ_find_items(BtechContext *context, DbRef store, int part, int brand) {
  (void)context;
  (void)store;
  (void)part;
  (void)brand;
  return inventory_count;
}

void econ_change_items(BtechContext *context, DbRef store, int part, int brand,
                       int count) {
  (void)context;
  changed_store = store;
  changed_part = part;
  changed_brand = brand;
  changed_count = count;
}

int main(void) {
  GameObject objects[2] = {0};
  GameDatabase database = {.objects = objects};
  BtechContext context = {.database = &database};
  MECH mech = {.xcode.context = &context, .mynum = 1};
  constexpr int part = 777;

  MechType(&mech) = CLASS_MECH;
  objects[1].location = 42;
  if (!mech_parts_available(&mech, part, 4, 6) ||
      mech_parts_available(&mech, part, 4, 7)) {
    return 1;
  }

  mech_parts_take(&mech, part, 4, 2);
  if (changed_store != 42 || changed_part != part || changed_brand != 4 ||
      changed_count != -2) {
    return 1;
  }

  mech_parts_add(&mech, 3, part, 5, 2);
#ifdef BT_COMPLEXREPAIRS
  constexpr int expected_part = part + 3;
#else
  constexpr int expected_part = part;
#endif
  return changed_store == 42 && changed_part == expected_part &&
                 changed_brand == 5 && changed_count == 2
             ? 0
             : 1;
}
