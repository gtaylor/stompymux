/* Declares the BattleTech unit tech commands API. */

#pragma once

#include "mux/server/platform.h"

typedef struct RepairCriticalSelection {
  Mech *mech;
  int location;
  int position;
} RepairCriticalSelection;

/* mech.tech.commands.c */
int someone_repairing_s(Mech *mech, int loc, int part, int t);
int someone_repairing(Mech *mech, int loc, int part);
int someone_replacing_suit(Mech *mech, int loc);
int someone_fixing_a(Mech *mech, int loc);
int someone_fixing_i(Mech *mech, int loc);
int someone_fixing(Mech *mech, int loc);
int someone_attaching(Mech *mech, int loc);
int someone_resealing(Mech *mech, int loc);
int someone_scrapping_loc(Mech *mech, int loc);
int someone_scrapping_part(Mech *mech, int loc, int part);
int can_scrap_loc(Mech *mech, int loc);
int can_scrap_part(Mech *mech, int loc, int part);
bool valid_gun_pos(const RepairCriticalSelection *selection);
void tech_checkstatus(DbRef player, void *data, char *buffer);
void tech_removegun(DbRef player, void *data, char *buffer);
void tech_removepart(DbRef player, void *data, char *buffer);
int invalid_scrap_path(Mech *mech, int loc);
void tech_removesection(DbRef player, void *data, char *buffer);
void tech_replacegun(DbRef player, void *data, char *buffer);
void tech_repairgun(DbRef player, void *data, char *buffer);
void tech_fixenhcrit(DbRef player, void *data, char *buffer);
void tech_replacepart(DbRef player, void *data, char *buffer);
void tech_repairpart(DbRef player, void *data, char *buffer);
void tech_toggletype(DbRef player, void *data, char *buffer);
void tech_reload(DbRef player, void *data, char *buffer);
void tech_unload(DbRef player, void *data, char *buffer);
void tech_fixarmor(DbRef player, void *data, char *buffer);
void tech_fixinternal(DbRef player, void *data, char *buffer);
int invalid_repair_path(Mech *mech, int loc);
int unit_is_fixable(Mech *mech);
void tech_reattach(DbRef player, void *data, char *buffer);
void tech_reseal(DbRef player, void *data, char *buffer);
void tech_magic(DbRef player, void *data, char *buffer);
void tech_fixextra(DbRef player, void *data, char *buffer);
void tech_replacesuit(DbRef player, void *data, char *buffer);
