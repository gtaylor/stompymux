
/* p.mech.ammodump.h */

#pragma once

#include "mux/server/platform.h"

int mech_ammunition_dump_decrease(Mech *mech, int loc, int pos, int *hm);
void mech_dump(DbRef player, void *data, char *buffer);
void mech_ammunition_dump_explode(Mech *mech, Mech *attacker, int w_hit_loc);
typedef struct BtechContext BtechContext;

int weapon_maximum_ammunition_damage(BtechContext *context, int weapon_index);

typedef struct DumpingAmmunitionItem {
  int damage;
  int location;
  int slot;
  int weapon_index;
  int part_type;
} DumpingAmmunitionItem;
