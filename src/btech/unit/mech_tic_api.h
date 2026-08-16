/* Declares the BattleTech unit tic API. */

#pragma once

#include <stdbool.h>

#include "mux/server/platform.h"

typedef struct Mech Mech;

/* mech.tic.c */
void cleartic_sub(DbRef player, Mech *mech, char *buffer);
void addtic_sub(DbRef player, Mech *mech, char *buffer);
void deltic_sub(DbRef player, Mech *mech, char *buffer);
void firetic_sub(DbRef player, Mech *mech, char *buffer);
void listtic_sub(DbRef player, Mech *mech, char *buffer);
void mech_cleartic(DbRef player, Mech *mech, char *buffer);
void mech_addtic(DbRef player, Mech *mech, char *buffer);
void mech_deltic(DbRef player, Mech *mech, char *buffer);
void mech_firetic(DbRef player, Mech *mech, char *buffer);
void mech_listtic(DbRef player, Mech *mech, char *buffer);
void heat_cutoff(DbRef player, Mech *mech, char *buffer);
typedef struct TicWeaponReference {
  int tic;
  int weapon;
} TicWeaponReference;
bool mech_tic_contains_weapon(const Mech *mech, TicWeaponReference reference);
