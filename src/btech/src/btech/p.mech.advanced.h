
/*
   p.mech.advanced.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Wed Feb 17 23:36:31 CET 1999 from mech.advanced.c */

#pragma once

/* mech.advanced.c */
void mech_ecm(DbRef player, MECH *mech, char *buffer);
void mech_eccm(DbRef player, MECH *mech, char *buffer);
void mech_perecm(DbRef player, MECH *mech, char *buffer);
void mech_pereccm(DbRef player, MECH *mech, char *buffer);
void mech_angelecm(DbRef player, MECH *mech, char *buffer);
void mech_angeleccm(DbRef player, MECH *mech, char *buffer);
void mech_stinger(DbRef player, MECH *mech, char *buffer);
void mech_slite(DbRef player, MECH *mech, char *buffer);
void mech_ams(DbRef player, void *data, char *buffer);
void mech_fliparms(DbRef player, void *data, char *buffer);
void mech_flamerheat(DbRef player, void *data, char *buffer);
void mech_ultra(DbRef player, void *data, char *buffer);
void mech_rac(DbRef player, void *data, char *buffer);
void mech_rapidfire(DbRef player, void *data, char *buffer);
void mech_unjamammo(DbRef player, void *data, char *buffer);
void mech_gattling(DbRef player, void *data, char *buffer);
void mech_inarc_ammo_toggle(DbRef player, void *data, char *buffer);
void mech_explosive(DbRef player, void *data, char *buffer);
void mech_lbx(DbRef player, void *data, char *buffer);
void mech_armorpiercing(DbRef player, void *data, char *buffer);
void mech_flechette(DbRef player, void *data, char *buffer);
void mech_incendiary(DbRef player, void *data, char *buffer);
void mech_precision(DbRef player, void *data, char *buffer);
void mech_caseless(DbRef plyaer, void *data, char *buffer);
void mech_artemis(DbRef player, void *data, char *buffer);
void mech_narc(DbRef player, void *data, char *buffer);
void mech_swarm(DbRef player, void *data, char *buffer);
void mech_swarm1(DbRef player, void *data, char *buffer);
void mech_inferno(DbRef player, void *data, char *buffer);
void mech_hotload(DbRef player, void *data, char *buffer);
void mech_cluster(DbRef player, void *data, char *buffer);
void mech_smoke(DbRef player, void *data, char *buffer);
void mech_mine(DbRef player, void *data, char *buffer);
void mech_masc(DbRef player, void *data, char *buffer);
void mech_scharge(DbRef player, void *data, char *buffer);
void mech_explode(DbRef player, void *data, char *buffer);
void mech_dig(DbRef player, void *data, char *buffer);
void mech_fixturret(DbRef player, void *data, char *buffer);
void mech_disableweap(DbRef player, void *data, char *buffer);
int FindMainWeapon(MECH *mech, int (*callback)(MECH *, int, int, int, int));
void mech_stealtharmor(DbRef player, MECH *mech, char *buffer);
void mech_nullsig(DbRef player, MECH *mech, char *buffer);
void show_narc_pods(DbRef player, MECH *mech, char *buffer);
void remove_inarc_pods_mech(DbRef player, MECH *mech, char *buffer);
void remove_inarc_pods_tank(DbRef player, MECH *mech, char *buffer);
void mech_auto_turret(DbRef player, MECH *mech, char *buffer);
void mech_usebin(DbRef player, MECH *mech, char *buffer);
void mech_safety(DbRef player, void *data, char *buffer);
void mech_mechprefs(DbRef player, void *data, char *buffer);
