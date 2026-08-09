/* Declares the BattleTech econ API. */

#pragma once

#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;
typedef struct BtechContext BtechContext;

/* econ.c */
void econ_change_items(BtechContext *context, DbRef d, int id, int brand,
                       int num);
int econ_find_items(BtechContext *context, DbRef d, int id, int brand);
void econ_set_items(BtechContext *context, DbRef d, int id, int brand, int num);
