/* Declares the BattleTech luck API. */

#pragma once

#include "mux/server/platform.h"

/* luck.c */
int player_luck(DbRef player);
int luck_die_mod_base(int mod, int l);
int luck_die_mod(DbRef player, int mod);
