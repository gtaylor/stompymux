#include "mech_parts.h"

#include "btech/context.h"
#include "btech/core/context_internal.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_internal.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mux/objects/db.h"

static int inventory_count = 6;
static DbRef changed_store;
static int changed_part;
static int changed_brand;
static int changed_count;

int econ_find_items(BtechContext *context, DbRef store, int part, int brand);

int econ_find_items(BtechContext *context [[maybe_unused]],
                    DbRef store [[maybe_unused]], int part [[maybe_unused]],
                    int brand [[maybe_unused]]) {
  return inventory_count;
}

void economy_inventory_change(const EconomyInventoryChange *change) {
  (void)change->context;
  changed_store = change->store;
  changed_part = change->part.id;
  changed_brand = change->part.brand;
  changed_count = change->quantity_delta;
}

static bool alias_matches(Mech *mech, int technology, int part, int cargo) {
  mech_technology_flags_set(mech, technology);
  return mech_parts_alias(mech, special_equipment_index(part)) ==
         cargo_equipment_index(cargo);
}

int main(void) {
  GameObject object_storage[3] = {0};
  GameDatabase database = {.object_storage = object_storage, .size = 2};
  BtechContext context = {.database = &database};
  Mech mech = {.xcode.context = &context, .mynum = 1};
  constexpr int part = 777;

  mech_class_set(&mech, CLASS_VTOL);
  mech_movement_type_set(&mech, MOVE_VTOL);
  if (mech_class(&mech) != CLASS_VTOL || mech.ud.type != CLASS_VTOL ||
      mech_movement_type(&mech) != MOVE_VTOL || mech.ud.move != MOVE_VTOL) {
    return 1;
  }
  mech_class_set(&mech, CLASS_MECH);
  mech_movement_type_set(&mech, MOVE_BIPED);
  if (!alias_matches(&mech, 0, SHOULDER_OR_HIP, S_ACTUATOR) ||
      !alias_matches(&mech, XL_TECH, ENGINE, XL_ENGINE) ||
      !alias_matches(&mech, ICE_TECH, ENGINE, IC_ENGINE) ||
      !alias_matches(&mech, XXL_TECH, ENGINE, XXL_ENGINE) ||
      !alias_matches(&mech, CE_TECH, ENGINE, COMP_ENGINE) ||
      !alias_matches(&mech, LE_TECH, ENGINE, LIGHT_ENGINE) ||
      !alias_matches(&mech, DOUBLE_HEAT_TECH, HEAT_SINK, DOUBLE_HEAT_SINK) ||
      !alias_matches(&mech, CLAN_TECH, HEAT_SINK, DOUBLE_HEAT_SINK)) {
    return 1;
  }
  game_database_object(&database, 1)->location = 42;
  if (!mech_parts_available(&mech, part, 4, 6) ||
      mech_parts_available(&mech, part, 4, 7)) {
    return 1;
  }

  mech_parts_take(&mech, part, 4, 2);
  if (changed_store != 42 || changed_part != part || changed_brand != 4 ||
      changed_count != -2) {
    return 1;
  }

  mech_parts_add(&mech, part, 5, 2);
  return changed_store == 42 && changed_part == part && changed_brand == 5 &&
                 changed_count == 2
             ? 0
             : 1;
}
