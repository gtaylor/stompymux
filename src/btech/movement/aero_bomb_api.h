
/*
   p.aero.bomb.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:32 CET 1999 from aero.bomb.c */

#pragma once

#include "mux/server/platform.h"

/* aero.bomb.c */
void DestroyBomb(Mech *mech, int loc);
int BombWeight(int i);
const char *bomb_name(int i);
void bomb_list(Mech *mech, int player);
float calc_dest(Mech *mech, short *x, short *y);
void bomb_aim(Mech *mech, DbRef player);
void bomb_hit_hexes(BattleMap *map, int x, int y, int hitnb, int iscluster,
                    int aff_d, int aff_h, char *tomsg, char *otmsg,
                    char *tomsg1, char *otmsg1);
void simulate_flight(Mech *mech, BattleMap *map, short *x, short *y, float t);
void bomb_drop(Mech *mech, int player, int bn);
void mech_bomb(DbRef player, void *data, char *buffer);
