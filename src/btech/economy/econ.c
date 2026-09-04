
/* Implements the BattleTech economy commands. */

#include "btech/context.h"
#include "context_internal.h" // IWYU pragma: keep
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_partnames_api.h"
#include "mux/objects/economy_parts.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

// NOLINTNEXTLINE(misc-no-recursion)
void economy_inventory_change(const EconomyInventoryChange *change) {
  BtechContext *context = change->context;
  const DbRef D = change->store;
  const int ID = change->part.id;
  int brand = change->part.brand;
  const int NUM = change->quantity_delta;
  GameDatabase *database = context->database;
  int base;

  if (!is_good_obj(database, D))
    return;
  if (brand)
    if (get_parts_short_name(context, ID, brand) ==
        get_parts_short_name(context, ID, 0))
      brand = 0;
  base = economy_parts_quantity(database, D, ID, brand);
  base += NUM;
  if (base <= 0) {
    economy_parts_set_quantity(database, D, ID, brand, 0);
    return;
  }
  if (!(equipment_is_actuator(ID)))
    economy_parts_set_quantity(database, D, ID, brand, base);
  if (equipment_is_actuator(ID)) {
    economy_inventory_change(&(EconomyInventoryChange){
        .context = context,
        .store = D,
        .part = {.id = cargo_equipment_index(S_ACTUATOR), .brand = brand},
        .quantity_delta = base,
    });
  }
  /* Successfully changed */
}

int econ_find_items(BtechContext *context, DbRef d, int id, int brand) {
  GameDatabase *database = context->database;
  if (!is_good_obj(database, d))
    return 0;
  if (brand)
    if (get_parts_short_name(context, id, brand) ==
        get_parts_short_name(context, id, 0))
      brand = 0;
  return economy_parts_quantity(database, d, id, brand);
}

void econ_set_items(BtechContext *context, DbRef d, int id, int brand,
                    int num) {
  int i;

  if (!is_good_obj(context->database, d))
    return;
  i = econ_find_items(context, d, id, brand);
  if (i != num) {
    economy_inventory_change(&(EconomyInventoryChange){
        .context = context,
        .store = d,
        .part = {.id = id, .brand = brand},
        .quantity_delta = num - i,
    });
  }
}
