/* Declares the BattleTech econ API. */

#pragma once

#include "mech_partnames_api.h"
#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;
typedef struct BtechContext BtechContext;

/* econ.c */
typedef struct EconomyInventoryChange {
  BtechContext *context;
  DbRef store;
  PartReference part;
  int quantity_delta;
} EconomyInventoryChange;
void economy_inventory_change(const EconomyInventoryChange *change);
int econ_find_items(BtechContext *context, DbRef d, int id, int brand);
void econ_set_items(BtechContext *context, DbRef d, int id, int brand, int num);
