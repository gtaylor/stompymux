/* Declares the BattleTech unit tic API. */

#pragma once

#include <stdbool.h>

#include "mux/server/platform.h"

typedef struct Mech Mech;

/* mech.tic.c */
int cleartic_sub_func(Mech *mech, DbRef player, int low, int high,
                      void *context);
void cleartic_sub(DbRef player, Mech *mech, char *buffer);
int addtic_sub_func(Mech *mech, DbRef player, int low, int high, void *context);
void addtic_sub(DbRef player, Mech *mech, char *buffer);
int deltic_sub_func(Mech *mech, DbRef player, int low, int high, void *context);
void deltic_sub(DbRef player, Mech *mech, char *buffer);
int firetic_sub_func(Mech *mech, DbRef player, int low, int high,
                     void *context);
void firetic_sub(DbRef player, Mech *mech, char *buffer);
void listtic_sub(DbRef player, Mech *mech, char *buffer);
void mech_cleartic(DbRef player, void *data, char *buffer);
void mech_addtic(DbRef player, void *data, char *buffer);
void mech_deltic(DbRef player, void *data, char *buffer);
void mech_firetic(DbRef player, void *data, char *buffer);
void mech_listtic(DbRef player, void *data, char *buffer);
void heat_cutoff(DbRef player, void *data, char *buffer);
bool mech_tic_contains_weapon(const Mech *mech, int tic, int weapon_number);
