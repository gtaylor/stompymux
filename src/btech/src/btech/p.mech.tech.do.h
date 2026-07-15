
/*
   p.mech.tech.do.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:59 CET 1999 from mech.tech.do.c */

#pragma once

/* mech.tech.do.c */
int valid_ammo_mode(MECH *mech, int loc, int part, int let);
int FindAmmoType(MECH *mech, int loc, int part);
int replace_econ(DbRef player, MECH *mech, int loc, int part);
int reload_econ(DbRef player, MECH *mech, int loc, int part, int *val);
int fixarmor_econ(DbRef player, MECH *mech, int loc, int *val);
int fixinternal_econ(DbRef player, MECH *mech, int loc, int *val);
int repair_econ(DbRef player, MECH *mech, int loc, int part);
int repairenhcrit_econ(DbRef player, MECH *mech, int loc, int part);
int reattach_econ(DbRef player, MECH *mech, int loc);
int replacesuit_econ(DbRef player, MECH *mech, int loc);
int reseal_econ(DbRef player, MECH *mech, int loc);
int replacep_succ(DbRef player, MECH *mech, int loc, int part);
int replaceg_succ(DbRef player, MECH *mech, int loc, int part);
int reload_succ(DbRef player, MECH *mech, int loc, int part, int *val);
int fixinternal_succ(DbRef player, MECH *mech, int loc, int *val);
int fixarmor_succ(DbRef player, MECH *mech, int loc, int *val);
int reattach_succ(DbRef player, MECH *mech, int loc);
int replacesuit_succ(DbRef player, MECH *mech, int loc);
int reseal_succ(DbRef player, MECH *mech, int loc);
int repairg_succ(DbRef player, MECH *mech, int loc, int part);
int repairenhcrit_succ(DbRef player, MECH *mech, int loc, int part);
int repairp_succ(DbRef player, MECH *mech, int loc, int part);
int replacep_fail(DbRef player, MECH *mech, int loc, int part);
int repairp_fail(DbRef player, MECH *mech, int loc, int part);
int replaceg_fail(DbRef player, MECH *mech, int loc, int part);
int repairg_fail(DbRef player, MECH *mech, int loc, int part);
int repairenhcrit_fail(DbRef player, MECH *mech, int loc, int part);
int reload_fail(DbRef player, MECH *mech, int loc, int part, int *val);
int fixarmor_fail(DbRef player, MECH *mech, int loc, int *val);
int fixinternal_fail(DbRef player, MECH *mech, int loc, int *val);
int reattach_fail(DbRef player, MECH *mech, int loc);
int replacesuit_fail(DbRef player, MECH *mech, int loc);
int reseal_fail(DbRef player, MECH *mech, int loc);
