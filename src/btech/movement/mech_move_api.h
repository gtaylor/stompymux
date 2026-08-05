
/*
   p.mech.move.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:15 CET 1999 from mech.move.c */

#pragma once

#include "mux/server/platform.h"

/* mech.move.c */
const char *LateralDesc(Mech *mech);
void mech_lateral(DbRef player, void *data, char *buffer);
void mech_turnmode(DbRef player, void *data, char *buffer);
void mech_bootlegger(DbRef player, void *data, char *buffer);
void mech_eta(DbRef player, void *data, char *buffer);
float MechCargoMaxSpeed(Mech *mech, float mspeed);
float mech_effective_maximum_speed(Mech *mech);
void mech_drop(DbRef player, void *data, char *buffer);
void mech_stand(DbRef player, void *data, char *buffer);
void mech_land(DbRef player, void *data, char *buffer);
void mech_heading(DbRef player, void *data, char *buffer);
void mech_turret(DbRef player, void *data, char *buffer);
void mech_rotatetorso(DbRef player, void *data, char *buffer);
void mech_speed(DbRef player, void *data, char *buffer);
void mech_vertical(DbRef player, void *data, char *buffer);
void mech_thrash(DbRef player, void *data, char *buffer);
void mech_jump(DbRef player, void *data, char *buffer);
void mech_hulldown(DbRef player, void *data, char *buffer);
#ifdef BT_MOVEMENT_MODES
void mech_sprint(DbRef player, void *data, char *buffer);
void mech_evade(DbRef player, void *data, char *buffer);
void mech_dodge(DbRef player, void *date, char *buffer);
#endif
int DropGetElevation(Mech *mech);
void DropSetElevation(Mech *mech, int wantdrop);
void LandMech(Mech *mech);
void MechFloodsLoc(Mech *mech, int loc, int lev);
void MechFloods(Mech *mech);
void MechFalls(Mech *mech, int levels, int seemsg);
typedef enum MechDominoMode {
  MECH_DOMINO_GROUND,
  MECH_DOMINO_JUMP,
  MECH_DOMINO_FALL,
} MechDominoMode;

int battle_map_mech_count_in_hex(BattleMap *map, int x, int y, int friendly,
                                 int team);
int mech_domino_resolve(Mech *mech, MechDominoMode mode);
