
/*
   p.mech.move.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:15 CET 1999 from mech.move.c */

#pragma once

/* mech.move.c */
const char *LateralDesc(MECH *mech);
void mech_lateral(DbRef player, void *data, char *buffer);
void mech_turnmode(DbRef player, void *data, char *buffer);
void mech_bootlegger(DbRef player, void *data, char *buffer);
void mech_eta(DbRef player, void *data, char *buffer);
float MechCargoMaxSpeed(MECH *mech, float mspeed);
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
int DropGetElevation(MECH *mech);
void DropSetElevation(MECH *mech, int wantdrop);
void LandMech(MECH *mech);
void MechFloodsLoc(MECH *mech, int loc, int lev);
void MechFloods(MECH *mech);
void MechFalls(MECH *mech, int levels, int seemsg);
int mechs_in_hex(MAP *map, int x, int y, int friendly, int team);
void cause_damage(MECH *att, MECH *mech, int dam, int table);
int domino_space_in_hex(MAP *map, MECH *me, int x, int y, int friendly,
                        int mode, int cnt);
int domino_space(MECH *mech, int mode);
